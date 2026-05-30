use pinocchio::{AccountView, Address, error::ProgramError};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Escrow {
    maker: [u8; 32],
    mint_a: [u8; 32],
    mint_b: [u8; 32],
    amount_to_receive: [u8; 8],
    amount_to_give: [u8; 8],
    pub bump: u8,
}

impl Escrow {
    pub const LEN: usize = 32 + 32 + 32 + 8 + 8 + 1;

    pub fn from_account_info(account_info: &mut AccountView) -> Result<&mut Self, ProgramError> {
        let data = unsafe { account_info.borrow_unchecked_mut() };
        if data.len() != Escrow::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self) })
    }

    pub fn maker(&self) -> &Address {
        unsafe { &*(&self.maker as *const [u8; 32] as *const Address) }
    }

    pub fn maker_raw(&self) -> &[u8; 32] {
        &self.maker
    }

    pub fn set_maker(&mut self, maker: &Address) {
        self.maker.copy_from_slice(maker.as_ref());
    }

    pub fn mint_a(&self) -> &Address {
        unsafe { &*(&self.mint_a as *const [u8; 32] as *const Address) }
    }

    pub fn set_mint_a(&mut self, mint_a: &Address) {
        self.mint_a.copy_from_slice(mint_a.as_ref());
    }

    pub fn mint_b(&self) -> &Address {
        unsafe { &*(&self.mint_b as *const [u8; 32] as *const Address) }
    }

    pub fn set_mint_b(&mut self, mint_b: &Address) {
        self.mint_b.copy_from_slice(mint_b.as_ref());
    }

    pub fn amount_to_receive(&self) -> u64 {
        u64::from_le_bytes(self.amount_to_receive)
    }

    pub fn set_amount_to_receive(&mut self, amount: u64) {
        self.amount_to_receive = amount.to_le_bytes();
    }

    pub fn amount_to_give(&self) -> u64 {
        u64::from_le_bytes(self.amount_to_give)
    }

    pub fn set_amount_to_give(&mut self, amount: u64) {
        self.amount_to_give = amount.to_le_bytes();
    }
}
