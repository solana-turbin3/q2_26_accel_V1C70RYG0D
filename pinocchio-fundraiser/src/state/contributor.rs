use pinocchio::{AccountView, error::ProgramError};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contributor {
    amount: [u8; 8],
    pub bump: u8,
}

impl Contributor {
    pub const LEN: usize = 8 + 1;

    pub fn from_account_info(account: &mut AccountView) -> Result<&mut Self, ProgramError> {
        let data = unsafe { account.borrow_unchecked_mut() };
        if data.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self) })
    }

    pub fn amount(&self) -> u64 {
        u64::from_le_bytes(self.amount)
    }

    pub fn set_amount(&mut self, amount: u64) {
        self.amount = amount.to_le_bytes();
    }
}
