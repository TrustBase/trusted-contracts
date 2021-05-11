// Copyright 2019 TrustBase Network
// This file is part of TrustBase library.
//
// The TrustBase library is free software: you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// The TrustBase library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Lesser General Public License for more details.


#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

pub const TOKENID_INIT: u32 = 188;
pub const MATEDATA_INIT: u32 = 20;

#[ink::contract]
mod baseNFT {
    #[cfg(not(feature = "ink-as-dependency"))]
    use ink_storage::collections::{
        hashmap::Entry,
        HashMap as StorageHashMap,
    };
    use scale::{
        Decode,
        Encode,
    };
    use crate::{TOKENID_INIT,MATEDATA_INIT};

    /// A token ID.
    pub type TokenId = u32;

    #[ink(storage)]
    #[derive(Default)]
    pub struct Simple_NFT {
        /// Mapping from token to owner.
        token_owner: StorageHashMap<TokenId, AccountId>,
        /// Mapping from owner to number of owned token.
        owned_tokens_count: StorageHashMap<AccountId, u32>,
        /// mapping from token to matedata
        matedatas: StorageHashMap<TokenId, u32>,
        /// mapping from token to approvals user
        /// (owner,tokenid) -> user
        approvals_token: StorageHashMap<(AccountId, TokenId), AccountId>,
    }

    #[derive(Encode, Decode, Debug, PartialEq, Eq, Copy, Clone)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        NotOwner,
        NotApproved,
        TokenExists,
        TokenNotFound,
        CannotInsert,
        CannotRemove,
        CannotFetchValue,
        NotAllowed,
    }

    /// Event emitted when a token transfer occurs.
    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        from: Option<AccountId>,
        #[ink(topic)]
        to: Option<AccountId>,
        #[ink(topic)]
        id: TokenId,
    }

    /// Event emitted when a token approve occurs.
    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        #[ink(topic)]
        id: TokenId,
    }

    /// Event emitted when an operator is enabled or disabled for an owner.
    /// The operator can manage all NFTs of the owner.
    #[ink(event)]
    pub struct ApprovalForAll {
        #[ink(topic)]
        owner: AccountId,
        #[ink(topic)]
        operator: AccountId,
        approved: bool,
    }

    impl Simple_NFT {
        /// Creates a new ERC721 token contract.
        #[ink(constructor)]
        pub fn new() -> Self {
            let mut my = Self {
                token_owner: Default::default(),
                owned_tokens_count: Default::default(),
                matedatas: Default::default(),
                approvals_token: Default::default(),
            };
            my.inherent_init();
            my
        }

        /// Returns the balance of the owner.
        ///
        /// This represents the amount of unique tokens the owner has.
        #[ink(message)]
        pub fn balance_of(&self, owner: AccountId) -> u32 {
            self.balance_of_or_zero(&owner)
        }

        /// Returns the owner of the token.
        #[ink(message)]
        pub fn owner_of(&self, id: TokenId) -> Option<AccountId> {
            self.token_owner.get(&id).cloned()
        }

        /// Returns the approved account ID for this token if any.
        #[ink(message)]
        pub fn get_approved(&self, id: TokenId) -> Option<AccountId> {
            let owner = self.owner_of(id);
            self
                .approvals_token
                .get(&(owner.expect("Error with AccountId"),id))
                .cloned()
        }

        /// Returns `true` if the operator is approved by the owner.
        #[ink(message)]
        pub fn is_approved(&self, id: TokenId, user: AccountId) -> bool {
            self.approved_for_token(id, user)
        }

        /// Approves the account to transfer the specified token on behalf of the caller.
        /// the last user will be valid
        #[ink(message)]
        pub fn approve(&mut self, to: AccountId, id: TokenId) -> Result<(), Error> {
            self.approve_for(&to, id)?;
            Ok(())
        }

        /// Transfers the token from the caller to the given destination.
        #[ink(message)]
        pub fn transfer(
            &mut self,
            destination: AccountId,
            id: TokenId,
        ) -> Result<(), Error> {
            let caller = self.env().caller();
            self.transfer_token_from(&caller, &destination, id,false)?;
            Ok(())
        }

        /// Transfer approved or owned token.
        #[ink(message)]
        pub fn transfer_from(
            &mut self,
            from: AccountId,
            to: AccountId,
            id: TokenId,
        ) -> Result<(), Error> {
            self.transfer_token_from(&from, &to, id,true)?;
            Ok(())
        }

        /// Transfers token `id` `from` the sender to the `to` AccountId.
        fn transfer_token_from(
            &mut self,
            from: &AccountId,
            to: &AccountId,
            id: TokenId,
            need_approval: bool,
        ) -> Result<(), Error> {
            let caller = self.env().caller();
            if !self.exists(id) {
                return Err(Error::TokenNotFound)
            };
            if !need_approval {
                let owner = self.owner_of(id);
                if !(owner == Some(caller)) {
                    return Err(Error::NotAllowed)
                }
            }
            if need_approval && !self.approved_for_token(id,caller) {
                return Err(Error::NotApproved)
            };
            self.clear_approval(id)?;
            self.remove_token_from(from, id)?;
            self.add_token_to(to, id)?;
            self.env().emit_event(Transfer {
                from: Some(*from),
                to: Some(*to),
                id,
            });
            Ok(())
        }

        /// Removes token `id` from the owner.
        fn remove_token_from(
            &mut self,
            from: &AccountId,
            id: TokenId,
        ) -> Result<(), Error> {
            let Self {
                token_owner,
                owned_tokens_count,
                ..
            } = self;
            let occupied = match token_owner.entry(id) {
                Entry::Vacant(_) => return Err(Error::TokenNotFound),
                Entry::Occupied(occupied) => occupied,
            };
            decrease_counter_of(owned_tokens_count, from)?;
            occupied.remove_entry();
            Ok(())
        }

        /// Adds the token `id` to the `to` AccountID.
        fn add_token_to(&mut self, to: &AccountId, id: TokenId) -> Result<(), Error> {
            let Self {
                token_owner,
                owned_tokens_count,
                ..
            } = self;
            let vacant_token_owner = match token_owner.entry(id) {
                Entry::Vacant(vacant) => vacant,
                Entry::Occupied(_) => return Err(Error::TokenExists),
            };
            if *to == AccountId::from([0x0; 32]) {
                return Err(Error::NotAllowed)
            };
            let entry = owned_tokens_count.entry(*to);
            increase_counter_of(entry);
            vacant_token_owner.insert(*to);
            Ok(())
        }

        /// Approve the passed AccountId to transfer the specified token on behalf of the message's sender.
        fn approve_for(&mut self, to: &AccountId, id: TokenId) -> Result<(), Error> {
            let caller = self.env().caller();
            let owner = self.owner_of(id);
            if !(owner == Some(caller)) {
                return Err(Error::NotAllowed)
            };
            if *to == AccountId::from([0x0; 32]) {
                return Err(Error::NotAllowed)
            };

            self.approvals_token.insert((owner.expect("Error with AccountId"),id), *to);
            self.env().emit_event(Approval {
                from: caller,
                to: *to,
                id,
            });
            Ok(())
        }

        /// Removes existing approval from token `id`.
        fn clear_approval(&mut self, id: TokenId) -> Result<(), Error> {
            let owner = self.owner_of(id);
            if !self.approvals_token.contains_key(&(owner.expect("Error with AccountId"),id)) {
                return Ok(())
            };
            match self.approvals_token.take(&(owner.expect("Error with AccountId"),id)) {
                Some(_res) => Ok(()),
                None => Err(Error::CannotRemove),
            }
        }

        /// inherent initialization a NFT token (max 5 nft token)
        fn inherent_init(&mut self) {
            let caller = self.env().caller();
            for i in 0..10 {
                self.add_token_to(&caller,TOKENID_INIT+i);
                self.matedatas.insert(TOKENID_INIT+i, MATEDATA_INIT+i);
            }
        }
        /// Returns the total number of tokens from an account.
        fn balance_of_or_zero(&self, of: &AccountId) -> u32 {
            *self.owned_tokens_count.get(of).unwrap_or(&0)
        }

        /// check the approved for the user
        fn approved_for_token(&self,id: TokenId,user: AccountId) -> bool {
            if user == AccountId::from([0x0; 32]) {
                return false
            }
            let owner = self.owner_of(id);
            user == *self
                .approvals_token
                .get(&(owner.expect("Error with AccountId"),id))
                .unwrap_or(&AccountId::from([0x0; 32]))
        }
        /// Returns true if token `id` exists or false if it does not.
        fn exists(&self, id: TokenId) -> bool {
            self.token_owner.get(&id).is_some() && self.token_owner.contains_key(&id)
        }
    }

    fn decrease_counter_of(
        hmap: &mut StorageHashMap<AccountId, u32>,
        of: &AccountId,
    ) -> Result<(), Error> {
        let count = (*hmap).get_mut(of).ok_or(Error::CannotFetchValue)?;
        *count -= 1;
        Ok(())
    }

    /// Increase token counter from the `of` AccountId.
    fn increase_counter_of(entry: Entry<AccountId, u32>) {
        entry.and_modify(|v| *v += 1).or_insert(1);
    }

    /// Unit tests
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;
        use ink_env::{
            call,
            test,
        };
        use ink_lang as ink;

        #[ink::test]
        fn init_works() {
            let accounts =
                ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
                    .expect("Cannot get accounts");
            set_sender(accounts.alice);
            // Create a new contract instance.
            let mut nft_token = Simple_NFT::new();
            // Token 1 does not exists.
            assert_eq!(nft_token.owner_of(1), None);
            // Alice has owns tokens.
            assert_eq!(nft_token.balance_of(accounts.alice), 10);
            for i in 0..10 {
                assert_eq!(nft_token.owner_of(TOKENID_INIT+i), Some(accounts.alice));
            }
        }

        #[ink::test]
        fn transfer_works() {
            let accounts =
                ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
                    .expect("Cannot get accounts");
            set_sender(accounts.alice);
            // Create a new contract instance.
            let mut nft_token = Simple_NFT::new();
            let token_id = TOKENID_INIT + 0;
            // Alice owns all tokens
            assert_eq!(nft_token.balance_of(accounts.alice), 10);
            // Bob does not owns any token
            assert_eq!(nft_token.balance_of(accounts.bob), 0);
            // alice own the token
            assert_eq!(nft_token.owner_of(token_id), Some(accounts.alice));
            // The first Transfer event takes place
            assert_eq!(0, ink_env::test::recorded_events().count());
            // Alice transfers token 1 to Bob
            assert_eq!(nft_token.transfer(accounts.bob, token_id), Ok(()));
            // The second Transfer event takes place
            assert_eq!(1, ink_env::test::recorded_events().count());
            // bob own the token
            assert_eq!(nft_token.owner_of(token_id), Some(accounts.bob));
            // Bob owns token 1
            assert_eq!(nft_token.balance_of(accounts.bob), 1);
            assert_eq!(nft_token.balance_of(accounts.alice), 9);
        }

        #[ink::test]
        fn invalid_transfer_should_fail() {
            let accounts =
                ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
                    .expect("Cannot get accounts");
            set_sender(accounts.alice);
            // Create a new contract instance.
            let mut nft_token = Simple_NFT::new();
            let token_id = TOKENID_INIT + 0;
            // alice own the token.
            assert_eq!(nft_token.owner_of(token_id), Some(accounts.alice));

            // token(id=2) not exist
            assert_eq!(nft_token.owner_of(2), None);
            // Transfer token fails if it does not exists.
            assert_eq!(nft_token.transfer(accounts.bob, 2), Err(Error::TokenNotFound));

            set_sender(accounts.bob);
            // Bob cannot transfer not owned tokens.
            assert_eq!(nft_token.transfer(accounts.eve, token_id), Err(Error::NotAllowed));
        }

        #[ink::test]
        fn approved_transfer_works() {
            let accounts =
                ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
                    .expect("Cannot get accounts");
            set_sender(accounts.alice);
            // Create a new contract instance.
            let mut nft_token = Simple_NFT::new();
            let token_id = TOKENID_INIT + 0;
            // alice has all tokens (10)
            assert_eq!(nft_token.balance_of(accounts.alice), 10);
            // Token Id(token_id) is owned by Alice.
            assert_eq!(nft_token.owner_of(token_id), Some(accounts.alice));
            // Approve token Id(token_id) transfer for Bob on behalf of Alice.
            assert_eq!(nft_token.approve(accounts.bob, token_id), Ok(()));
            set_sender(accounts.bob);
            // Bob transfers token Id(token_id) from Alice to Eve.
            assert_eq!(
                nft_token.transfer_from(accounts.alice, accounts.eve, token_id),
                Ok(())
            );
            // TokenId Id(token_id) is owned by Eve.
            assert_eq!(nft_token.owner_of(token_id), Some(accounts.eve));
            // Alice has 9 tokens.
            assert_eq!(nft_token.balance_of(accounts.alice), 9);
            // Bob does not owns tokens.
            assert_eq!(nft_token.balance_of(accounts.bob), 0);
            // Eve owns 1 token.
            assert_eq!(nft_token.balance_of(accounts.eve), 1);
        }

        fn set_sender(sender: AccountId) {
            let callee = ink_env::account_id::<ink_env::DefaultEnvironment>()
                .unwrap_or([0x0; 32].into());
            test::push_execution_context::<Environment>(
                sender,
                callee,
                1000000,
                1000000,
                test::CallData::new(call::Selector::new([0x00; 4])), // dummy
            );
        }
    }
}
