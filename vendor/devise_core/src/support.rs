bitflags! {
    pub struct GenericSupport: u32 {
        const None     = 0b000;
        const Type     = 0b001;
        const Lifetime = 0b010;
        const Const    = 0b100;
        const All      = 0b111;
    }
}

bitflags! {
    pub struct DataSupport: u32 {
        const None        = 0b0000;
        const TupleStruct = 0b0001;
        const NamedStruct = 0b0010;
        const Struct      = 0b0011;
        const Enum        = 0b0100;
        const Union       = 0b1000;
        const All         = 0b1111;
    }
}
