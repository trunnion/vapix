use serde::{Deserialize, Serialize};

/// A system architecture used by an AXIS product.
///
/// This enumeration contains all known architectures. It is `#[non_exhaustive]` since it is
/// expected that AXIS will use additional architectures in the future.
///
/// `Architecture` encodes both a processor instruction set and ABI. For example, the AXIS ARTPEC-7
/// SoC could in principle run `Armv7Hf`, `Armv7`, `Armv6`, or `Armv5tej` software, but only one of
/// these will work in practice because the Linux kernel and `libc` were built for `Armv7Hf`.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Architecture {
    /// Aarch 64
    Aarch64,
    /// Arm v5 with Thumb, Enhanced DSP, and Jazelle, in little endian byte order, using the GNU
    /// Arm embedded ABI.
    Armv5tej,
    /// Arm v6 in little endian byte order, using the GNU Arm embedded ABI.
    Armv6,
    /// Arm v7 in little endian byte order, using the GNU Arm embedded ABI.
    Armv7,
    /// Arm v7 with hardware floating point, in little endian byte order, using the GNU Arm embedded
    /// ABI.
    Armv7Hf,
    /// CRIS v0–v10, i.e. chips up to and including ETRAX 100LX and ARTPEC-2.
    ///
    /// These version numbers are defined in the [ETRAX FS Designer's Reference].
    ///
    /// [ETRAX FS Designer's Reference]: https://www.axis.com/files/manuals/etrax_fs_des_ref-070821.pdf
    CrisV0,
    /// CRIS v32, as used in ETRAX FS and ARTPEC-3.
    CrisV32,
    /// MIPS 32-bit revision 2, in little endian byte order.
    Mips,
}

impl Architecture {
    /// List all architectures.
    pub fn all() -> &'static [Architecture] {
        &[
            Architecture::Aarch64,
            Architecture::Armv5tej,
            Architecture::Armv6,
            Architecture::Armv7,
            Architecture::Armv7Hf,
            Architecture::CrisV0,
            Architecture::CrisV32,
            Architecture::Mips,
        ]
    }

    /// The display name of this Architecture.
    pub fn display_name(&self) -> &'static str {
        match self {
            Architecture::Aarch64 => "AArch64",
            Architecture::Armv5tej => "ARMv5TEJ",
            Architecture::Armv6 => "ARMv6",
            Architecture::Armv7 => "ARMv7",
            Architecture::Armv7Hf => "ARMv7-HF",
            Architecture::CrisV0 => "CRISv0",
            Architecture::CrisV32 => "CRISv32",
            Architecture::Mips => "MIPS",
        }
    }

    pub(crate) fn from_param(value: &str) -> Option<Self> {
        Some(match value {
            "aarch64" => Self::Aarch64,
            "armv5tejl" => Self::Armv5tej,
            "armv6l" => Self::Armv6,
            "armv7l" => Self::Armv7,
            "armv7hf" => Self::Armv7Hf,
            "crisv0" => Self::CrisV0,
            "crisv32" => Self::CrisV32,
            "mips" => Self::Mips,
            _ => return None,
        })
    }

    /// Sniff an ELF executable, determining for which `Architecture` it was built.
    #[cfg(feature = "goblin")]
    pub fn sniff(executable: &[u8]) -> Option<Self> {
        use goblin::elf::arm::{ElfExt, HeaderExt};
        use goblin::elf::header::{EI_DATA, ELFDATA2LSB};
        use goblin::elf::header::{EM_AARCH64, EM_ARM, EM_CRIS, EM_MIPS};

        /* Variant 0; may contain v0..10 object. */
        const EF_CRIS_VARIANT_ANY_V0_V10: u32 = 0x0000_0000;
        /* Variant 1; contains v32 object.  */
        const EF_CRIS_VARIANT_V32: u32 = 0x0000_0002;
        /* Variant 2; contains object compatible with v32 and v10.  */
        const EF_CRIS_VARIANT_COMMON_V10_V32: u32 = 0x0000_0004;

        /* Four bit MIPS architecture field.  */
        const EF_MIPS_ARCH: u32 = 0xf000_0000;
        /* -mips32r2 code.  */
        const E_MIPS_ARCH_32R2: u32 = 0x7000_0000;

        let elf = match goblin::elf::Elf::parse(executable) {
            Ok(elf) => elf,
            Err(_) => return None,
        };

        match elf.header.e_machine {
            // Aarch64:
            EM_AARCH64 => Some(Architecture::Aarch64),
            // CRIS:
            EM_CRIS => match elf.header.e_flags {
                // v32 specific:
                EF_CRIS_VARIANT_V32 => Some(Architecture::CrisV32),
                // v0, or v10-compatible:
                EF_CRIS_VARIANT_ANY_V0_V10 | EF_CRIS_VARIANT_COMMON_V10_V32 => {
                    Some(Architecture::CrisV0)
                }
                // Other:
                _ => None,
            },
            // MIPS:
            EM_MIPS => match (
                elf.header.e_flags & EF_MIPS_ARCH,
                elf.header.e_ident[EI_DATA],
            ) {
                // mips32r2el:
                (E_MIPS_ARCH_32R2, ELFDATA2LSB) => Some(Architecture::Mips),
                // Other:
                _ => None,
            },
            // Arm, little endian:
            EM_ARM if elf.header.e_ident[EI_DATA] == ELFDATA2LSB => {
                // There's only one Arm hard-float arch
                // Let's decide that any hard-float image is Armv7-HF
                if let Some(arm_header) = elf.header.arm() {
                    if arm_header.is_hard_float() {
                        return Some(Architecture::Armv7Hf);
                    }
                } else {
                    // arm() returns Some() for all EM_ARM
                    unreachable!();
                }

                // ELF header says we're soft-float
                // Check the aeabi build attributes to discrimate architecture with specificity
                if let Ok(aeabi) = elf.aeabi(executable) {
                    use goblin::elf::build_attributes::aeabi::CpuArch;
                    match aeabi.cpu_arch {
                        // V5(T|TE|TEJ):
                        Some(CpuArch::V5t) | Some(CpuArch::V5te) | Some(CpuArch::V5tej) => {
                            Some(Architecture::Armv5tej)
                        }
                        // V6:
                        Some(CpuArch::V6) => return Some(Architecture::Armv6),
                        // V7:
                        Some(CpuArch::V7) => Some(Architecture::Armv7),
                        // Other:
                        _ => None,
                    }
                } else {
                    // No aeabi build attributes => no idea which Arm this is
                    None
                }
            }
            // Unknown machine type:
            _ => None,
        }
    }
}

impl std::fmt::Display for Architecture {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

/// A system-on-chip used by an AXIS product.
///
/// This enumeration contains all known SOCs. It is `#[non_exhaustive]` since it is expected that
/// AXIS will use additional SOCs in the future.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum SOC {
    Artpec1,
    Artpec2,
    Artpec3,
    Artpec4,
    Artpec5,
    Artpec6,
    Artpec7,
    A5S,
    Hi3516cV300,
    Hi3719cV100,
    MX8QP,
    S2,
    S2E,
    S2L,
    S3L,
    S5,
    S5L,
}

impl SOC {
    /// List all SoCs.
    pub fn all() -> &'static [SOC] {
        &[
            SOC::Artpec1,
            SOC::Artpec2,
            SOC::Artpec3,
            SOC::Artpec4,
            SOC::Artpec5,
            SOC::Artpec6,
            SOC::Artpec7,
            SOC::A5S,
            SOC::Hi3516cV300,
            SOC::Hi3719cV100,
            SOC::MX8QP,
            SOC::S2,
            SOC::S2E,
            SOC::S2L,
            SOC::S3L,
            SOC::S5,
            SOC::S5L,
        ]
    }

    /// The display name of this SoC.
    pub fn display_name(&self) -> &'static str {
        match self {
            SOC::Artpec1 => "Axis ARTPEC-1",
            SOC::Artpec2 => "Axis ARTPEC-2",
            SOC::Artpec3 => "Axis ARTPEC-3",
            SOC::Artpec4 => "Axis ARTPEC-4",
            SOC::Artpec5 => "Axis ARTPEC-5",
            SOC::Artpec6 => "Axis ARTPEC-6",
            SOC::Artpec7 => "Axis ARTPEC-7",
            SOC::A5S => "Ambarella A5S",
            SOC::Hi3516cV300 => "Hi3516C V300",
            SOC::Hi3719cV100 => "Hi3719C V100",
            SOC::MX8QP => "NXP i.MX 8 QP",
            SOC::S2 => "Ambarella S2",
            SOC::S2E => "Ambarella S2E",
            SOC::S2L => "Ambarella S2L",
            SOC::S3L => "Ambarella S3L",
            SOC::S5 => "Ambarella S5",
            SOC::S5L => "Ambarella S5L",
        }
    }

    pub(crate) fn from_param(value: &str) -> Option<Self> {
        Some(match value {
            "Axis Artpec-5" => Self::Artpec5,
            _ => return None,
        })
    }

    /// The year when this SoC was released.
    pub fn year(&self) -> u32 {
        match self {
            SOC::Artpec1 => 1999,
            SOC::Artpec2 => 2003,
            SOC::Artpec3 => 2007,
            SOC::Artpec4 => 2011,
            SOC::Artpec5 => 2013,
            SOC::Artpec6 => 2017,
            SOC::Artpec7 => 2019,
            SOC::A5S => 2010,
            SOC::Hi3516cV300 => 2016, //?
            SOC::Hi3719cV100 => 2016, //?
            SOC::MX8QP => 2013,
            SOC::S2 | SOC::S2E | SOC::S2L => 2012,
            SOC::S3L => 2014,
            SOC::S5 | SOC::S5L => 2016,
        }
    }

    /// The architecture most commonly used by this SoC.
    ///
    /// In principle, an SoC can support multiple architectures, varying by firmware image. In
    /// practice, Axis has compiled every firmware released for every product using a given SoC with
    /// the same architecture. Still, if you specifically need to know which architecture a given
    /// device is using, you should ask instead of assuming.
    pub fn architecture(&self) -> Architecture {
        match self {
            SOC::Artpec1 | SOC::Artpec2 | SOC::Artpec3 => Architecture::CrisV32,
            SOC::Artpec4 | SOC::Artpec5 => Architecture::Mips,
            SOC::Artpec6 | SOC::Artpec7 => Architecture::Armv7Hf,
            SOC::A5S => Architecture::Armv6,
            SOC::Hi3516cV300 => Architecture::Armv5tej,
            SOC::Hi3719cV100 => Architecture::Armv7Hf,
            SOC::MX8QP => Architecture::Aarch64,
            SOC::S2 => Architecture::Armv7,
            SOC::S2E | SOC::S2L => Architecture::Armv7Hf,
            SOC::S3L => Architecture::Armv7Hf,
            SOC::S5 | SOC::S5L => Architecture::Aarch64,
        }
    }
}

impl std::fmt::Display for SOC {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "goblin")]
    #[test]
    fn sniff() {
        for arch in Architecture::all() {
            let file = std::fs::read(format!("fixtures/elf/arch.{}", arch)).unwrap();
            assert_eq!(Architecture::sniff(&file), Some(*arch));
        }
    }
}
