// Base traits
pub trait A {}
pub trait B {}
pub trait C {}

// First level of inheritance
pub trait D: A {}
pub trait E: B + C {}

// Second level of inheritance
pub trait F: D + E {}
pub trait G: F + E + A {}

// Implementations
struct Type1;
impl A for Type1 {}
impl B for Type1 {}

struct Type2;
impl D for Type2 {}
impl E for Type2 {}

struct Type3;
impl F for Type3 {}

struct Type4;
impl G for Type4 {}

// This should create the following depths:
// Type1: depth 1 (implements A, B directly)
// Type2: depth 2 (implements D which requires A, and E which requires B+C)
// Type3: depth 3 (implements F which requires D+E, which require A,B,C) 
// Type 4: depth 4 (implements G which requires F+E+A, which require A,B,C,D,E,F)