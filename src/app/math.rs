#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Clone, Copy))]
pub(crate) enum Op {
    Add(u16),
    Sub(u16),
}

const MAGNITUDE: u16 = 5;

impl Op {
    pub(crate) fn exec(self, num: u16) -> u16 {
        let num = num / MAGNITUDE;
        let num = match self {
            Op::Add(n) => num.saturating_add(n),
            Op::Sub(n) => num.saturating_sub(n),
        };
        num * MAGNITUDE
    }
}
