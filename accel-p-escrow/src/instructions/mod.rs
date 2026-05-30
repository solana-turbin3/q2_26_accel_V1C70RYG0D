pub mod make;
pub mod refund;
pub mod take;
pub use make::*;
pub use refund::*;
pub use take::*;

use pinocchio::error::ProgramError;

pub enum EscrowInstructions {
    Make = 0,
    Refund = 1,
    Take = 2,
}

impl TryFrom<&u8> for EscrowInstructions {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(EscrowInstructions::Make),
            1 => Ok(EscrowInstructions::Refund),
            2 => Ok(EscrowInstructions::Take),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
