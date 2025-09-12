// Test file for Maximum Trait Depth (MTD) script
// This file contains various edge cases that expose parsing issues

// =============================================================================
// Basic trait hierarchy - should work correctly
// =============================================================================
pub trait A {}
pub trait B: A {}
pub trait C: B {}
struct BasicType;
impl C for BasicType {}
// Expected: BasicType should have depth 3 (C -> B -> A)

// =============================================================================
// Issue 1: Parser captures trait identifiers incorrectly
// =============================================================================
// The parser might capture "A{}" instead of "A" due to regex issues
pub trait SimpleA {}
pub trait SimpleB: SimpleA {}
struct SimpleType;
impl SimpleB for SimpleType {}
// Expected: SimpleType should have depth 2 (SimpleB -> SimpleA)

// Multiple bounds with potential parsing issues
pub trait MultiA {}
pub trait MultiB {}
pub trait MultiC: MultiA + MultiB {}
struct MultiType;
impl MultiC for MultiType {}
// Expected: MultiType should have depth 2 (MultiC -> MultiA, MultiB)

// =============================================================================
// Issue 2: Module path references
// =============================================================================
mod module_a { 
    pub trait TraitA {} 
}

mod module_b { 
    pub struct TypeB; 
}

// This should be captured but might be missed due to module path parsing
impl module_a::TraitA for module_b::TypeB {}

// Nested module paths
mod outer {
    pub mod inner {
        pub trait DeepTrait {}
    }
}

struct DeepType;
impl outer::inner::DeepTrait for DeepType {}

// =============================================================================
// Issue 3: Different visibility modifiers
// =============================================================================
// These might be missed if parser only looks for "trait" or "pub trait"

unsafe trait UnsafeTrait {}
struct UnsafeType;
unsafe impl UnsafeTrait for UnsafeType {}

pub(crate) trait CrateTrait {}
struct CrateType;
impl CrateTrait for CrateType {}

pub(super) trait SuperTrait {}
struct SuperType;
impl SuperTrait for SuperType {}

pub(in crate::module_a) trait RestrictedTrait {}
struct RestrictedType;
impl RestrictedTrait for RestrictedType {}

// =============================================================================
// Issue 4: Multiline trait declarations
// =============================================================================
// These are likely to be missed entirely
pub trait MultilineBase {}
pub trait MultilineHelper {}

pub trait MultilineTrait:
    MultilineBase + MultilineHelper
{}

struct MultilineType;
impl MultilineTrait for MultilineType {}
// Expected: MultilineType should have depth 2

// Even more complex multiline case
pub trait ComplexBase {}
pub trait ComplexHelper1 {}
pub trait ComplexHelper2 {}

pub trait ComplexMultiline:
    ComplexBase + 
    ComplexHelper1 +
    ComplexHelper2
{}

struct ComplexType;
impl ComplexMultiline for ComplexType {}

// =============================================================================
// Issue 5: Traits with generic parameters and where clauses
// =============================================================================
pub trait GenericBase<T> {}
pub trait GenericTrait<T>: GenericBase<T> 
where 
    T: Clone
{}

struct GenericType;
impl GenericBase<i32> for GenericType {}
impl GenericTrait<i32> for GenericType {}

// =============================================================================
// Issue 6: Complex inheritance chains that should test depth calculation
// =============================================================================
pub trait Level1 {}
pub trait Level2: Level1 {}
pub trait Level3: Level2 {}
pub trait Level4: Level3 {}
pub trait Level5: Level4 {}

struct DeepInheritanceType;
impl Level5 for DeepInheritanceType {}
// Expected: DeepInheritanceType should have depth 5

// Diamond inheritance pattern
pub trait DiamondBase {}
pub trait DiamondLeft: DiamondBase {}
pub trait DiamondRight: DiamondBase {}
pub trait DiamondTop: DiamondLeft + DiamondRight {}

struct DiamondType;
impl DiamondTop for DiamondType {}
// Expected: DiamondType should have depth 3

// =============================================================================
// Issue 7: Edge cases with formatting and whitespace
// =============================================================================
pub   trait   SpacedTrait   {}
struct SpacedType;
impl SpacedTrait for SpacedType {}

pub trait TabTrait	{}  // Contains tab character
struct TabType;
impl TabTrait for TabType {}

// Trait with comments
pub trait CommentedTrait {} // This is a comment
struct CommentedType;
impl CommentedTrait for CommentedType {}

// =============================================================================
// Issue 8: Traits in different contexts
// =============================================================================
// Trait in impl block
struct ContextType;
impl ContextType {
    // This shouldn't be captured as a trait declaration
    fn trait_method() {}
}

// Trait objects and dyn keywords (shouldn't be captured as trait declarations)
fn use_trait_object(_: &dyn SimpleA) {}
type TraitObjectType = Box<dyn SimpleA>;

// =============================================================================
// Expected Results Summary:
// =============================================================================
// BasicType: depth 3
// SimpleType: depth 2  
// MultiType: depth 2
// module_b::TypeB: depth 1
// DeepType: depth 1
// UnsafeType: depth 1
// CrateType: depth 1
// SuperType: depth 1
// RestrictedType: depth 1
// MultilineType: depth 2
// ComplexType: depth 2
// GenericType: depth 2
// DeepInheritanceType: depth 5
// DiamondType: depth 3
// SpacedType: depth 1
// TabType: depth 1
// CommentedType: depth 1
//
// Maximum expected depth: 5 (from DeepInheritanceType)