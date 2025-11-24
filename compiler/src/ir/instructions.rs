//! IR Instructions
//!
//! Defines the instruction set for the intermediate representation.
//! Instructions are low-level operations that map directly to machine operations.

use super::{IrId, IrType, IrValue, IrSourceLocation};

/// IR instruction
#[derive(Debug, Clone)]
pub enum IrInstruction {
    // === Value Operations ===
    
    /// Load constant value
    Const {
        dest: IrId,
        value: IrValue,
    },
    
    /// Copy value from one register to another
    Copy {
        dest: IrId,
        src: IrId,
    },
    
    /// Load value from memory
    Load {
        dest: IrId,
        ptr: IrId,
        ty: IrType,
    },
    
    /// Store value to memory
    Store {
        ptr: IrId,
        value: IrId,
    },
    
    // === Arithmetic Operations ===
    
    /// Binary arithmetic operation
    BinOp {
        dest: IrId,
        op: BinaryOp,
        left: IrId,
        right: IrId,
    },
    
    /// Unary operation
    UnOp {
        dest: IrId,
        op: UnaryOp,
        operand: IrId,
    },
    
    /// Compare operation
    Cmp {
        dest: IrId,
        op: CompareOp,
        left: IrId,
        right: IrId,
    },
    
    // === Control Flow ===
    
    /// Unconditional jump
    Jump {
        target: IrId,
    },
    
    /// Conditional branch
    Branch {
        condition: IrId,
        true_target: IrId,
        false_target: IrId,
    },
    
    /// Switch/jump table
    Switch {
        value: IrId,
        default_target: IrId,
        cases: Vec<(IrValue, IrId)>,
    },
    
    /// Function call
    Call {
        dest: Option<IrId>,
        func: IrId,
        args: Vec<IrId>,
    },
    
    /// Indirect function call
    CallIndirect {
        dest: Option<IrId>,
        func_ptr: IrId,
        args: Vec<IrId>,
        signature: IrType,
    },
    
    /// Return from function
    Return {
        value: Option<IrId>,
    },
    
    // === Memory Operations ===
    
    /// Allocate memory
    Alloc {
        dest: IrId,
        ty: IrType,
        count: Option<IrId>,
    },
    
    /// Free memory
    Free {
        ptr: IrId,
    },
    
    /// Get element pointer (GEP)
    GetElementPtr {
        dest: IrId,
        ptr: IrId,
        indices: Vec<IrId>,
        ty: IrType,
    },
    
    /// Memory copy
    MemCopy {
        dest: IrId,
        src: IrId,
        size: IrId,
    },
    
    /// Memory set
    MemSet {
        dest: IrId,
        value: IrId,
        size: IrId,
    },
    
    // === Type Operations ===
    
    /// Type cast
    Cast {
        dest: IrId,
        src: IrId,
        from_ty: IrType,
        to_ty: IrType,
    },
    
    /// Bitcast (reinterpret bits)
    BitCast {
        dest: IrId,
        src: IrId,
        ty: IrType,
    },
    
    // === Exception Handling ===
    
    /// Throw exception
    Throw {
        exception: IrId,
    },
    
    /// Begin exception handler
    LandingPad {
        dest: IrId,
        ty: IrType,
        clauses: Vec<LandingPadClause>,
    },
    
    /// Resume exception propagation
    Resume {
        exception: IrId,
    },
    
    // === Special Operations ===
    
    /// Phi node for SSA form
    Phi {
        dest: IrId,
        incoming: Vec<(IrId, IrId)>, // (value, predecessor block)
    },
    
    /// Select (ternary) operation
    Select {
        dest: IrId,
        condition: IrId,
        true_val: IrId,
        false_val: IrId,
    },
    
    /// Extract value from aggregate
    ExtractValue {
        dest: IrId,
        aggregate: IrId,
        indices: Vec<u32>,
    },
    
    /// Insert value into aggregate
    InsertValue {
        dest: IrId,
        aggregate: IrId,
        value: IrId,
        indices: Vec<u32>,
    },
    
    /// Debug location marker
    DebugLoc {
        location: IrSourceLocation,
    },
    
    /// Inline assembly
    InlineAsm {
        dest: Option<IrId>,
        asm: String,
        inputs: Vec<(String, IrId)>,
        outputs: Vec<(String, IrType)>,
        clobbers: Vec<String>,
    },
}

/// Binary operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    
    // Bitwise
    And,
    Or,
    Xor,
    Shl,
    Shr,
    
    // Floating point
    FAdd,
    FSub,
    FMul,
    FDiv,
    FRem,
}

/// Unary operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    // Arithmetic
    Neg,
    
    // Bitwise
    Not,
    
    // Floating point
    FNeg,
}

/// Comparison operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    // Integer comparisons
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    
    // Unsigned comparisons
    ULt,
    ULe,
    UGt,
    UGe,
    
    // Floating point comparisons
    FEq,
    FNe,
    FLt,
    FLe,
    FGt,
    FGe,
    
    // Floating point ordered/unordered
    FOrd,
    FUno,
}

/// Landing pad clause for exception handling
#[derive(Debug, Clone)]
pub enum LandingPadClause {
    /// Catch specific exception type
    Catch(IrType),
    /// Filter exceptions
    Filter(Vec<IrType>),
}

impl IrInstruction {
    /// Get the destination register if this instruction produces a value
    pub fn dest(&self) -> Option<IrId> {
        match self {
            IrInstruction::Const { dest, .. } |
            IrInstruction::Copy { dest, .. } |
            IrInstruction::Load { dest, .. } |
            IrInstruction::BinOp { dest, .. } |
            IrInstruction::UnOp { dest, .. } |
            IrInstruction::Cmp { dest, .. } |
            IrInstruction::Alloc { dest, .. } |
            IrInstruction::GetElementPtr { dest, .. } |
            IrInstruction::Cast { dest, .. } |
            IrInstruction::BitCast { dest, .. } |
            IrInstruction::LandingPad { dest, .. } |
            IrInstruction::Phi { dest, .. } |
            IrInstruction::Select { dest, .. } |
            IrInstruction::ExtractValue { dest, .. } |
            IrInstruction::InsertValue { dest, .. } => Some(*dest),
            
            IrInstruction::Call { dest, .. } |
            IrInstruction::CallIndirect { dest, .. } |
            IrInstruction::InlineAsm { dest, .. } => *dest,
            
            _ => None,
        }
    }
    
    /// Get all registers used by this instruction
    pub fn uses(&self) -> Vec<IrId> {
        match self {
            IrInstruction::Copy { src, .. } => vec![*src],
            IrInstruction::Load { ptr, .. } => vec![*ptr],
            IrInstruction::Store { ptr, value } => vec![*ptr, *value],
            IrInstruction::BinOp { left, right, .. } => vec![*left, *right],
            IrInstruction::UnOp { operand, .. } => vec![*operand],
            IrInstruction::Cmp { left, right, .. } => vec![*left, *right],
            IrInstruction::Branch { condition, .. } => vec![*condition],
            IrInstruction::Switch { value, .. } => vec![*value],
            IrInstruction::Call { func, args, .. } => {
                let mut uses = vec![*func];
                uses.extend(args);
                uses
            }
            IrInstruction::CallIndirect { func_ptr, args, .. } => {
                let mut uses = vec![*func_ptr];
                uses.extend(args);
                uses
            }
            IrInstruction::Return { value } => {
                value.map(|v| vec![v]).unwrap_or_default()
            }
            IrInstruction::Alloc { count, .. } => {
                count.map(|c| vec![c]).unwrap_or_default()
            }
            IrInstruction::Free { ptr } => vec![*ptr],
            IrInstruction::GetElementPtr { ptr, indices, .. } => {
                let mut uses = vec![*ptr];
                uses.extend(indices);
                uses
            }
            IrInstruction::MemCopy { dest, src, size } => vec![*dest, *src, *size],
            IrInstruction::MemSet { dest, value, size } => vec![*dest, *value, *size],
            IrInstruction::Cast { src, .. } |
            IrInstruction::BitCast { src, .. } => vec![*src],
            IrInstruction::Throw { exception } => vec![*exception],
            IrInstruction::Resume { exception } => vec![*exception],
            IrInstruction::Phi { incoming, .. } => {
                incoming.iter().map(|(val, _)| *val).collect()
            }
            IrInstruction::Select { condition, true_val, false_val, .. } => {
                vec![*condition, *true_val, *false_val]
            }
            IrInstruction::ExtractValue { aggregate, .. } => vec![*aggregate],
            IrInstruction::InsertValue { aggregate, value, .. } => vec![*aggregate, *value],
            IrInstruction::InlineAsm { inputs, .. } => {
                inputs.iter().map(|(_, id)| *id).collect()
            }
            _ => vec![],
        }
    }
    
    /// Check if this is a terminator instruction
    pub fn is_terminator(&self) -> bool {
        matches!(self,
            IrInstruction::Jump { .. } |
            IrInstruction::Branch { .. } |
            IrInstruction::Switch { .. } |
            IrInstruction::Return { .. } |
            IrInstruction::Throw { .. } |
            IrInstruction::Resume { .. }
        )
    }
    
    /// Check if this instruction has side effects
    pub fn has_side_effects(&self) -> bool {
        matches!(self,
            IrInstruction::Store { .. } |
            IrInstruction::Call { .. } |
            IrInstruction::CallIndirect { .. } |
            IrInstruction::Free { .. } |
            IrInstruction::MemCopy { .. } |
            IrInstruction::MemSet { .. } |
            IrInstruction::Throw { .. } |
            IrInstruction::InlineAsm { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_instruction_properties() {
        let add = IrInstruction::BinOp {
            dest: IrId::new(1),
            op: BinaryOp::Add,
            left: IrId::new(2),
            right: IrId::new(3),
        };
        
        assert_eq!(add.dest(), Some(IrId::new(1)));
        assert_eq!(add.uses(), vec![IrId::new(2), IrId::new(3)]);
        assert!(!add.is_terminator());
        assert!(!add.has_side_effects());
        
        let ret = IrInstruction::Return {
            value: Some(IrId::new(1)),
        };
        
        assert!(ret.is_terminator());
        assert_eq!(ret.uses(), vec![IrId::new(1)]);
    }
}