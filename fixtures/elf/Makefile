# Targets run on the host:
all_in_docker:
	docker run -it --rm -v `pwd`:/elf -w /elf trunnion/cargo-acap:1.47.0 make all_cargo_acap
	docker build -t vapix-fedora-cris .
	docker run -it --rm -v `pwd`:/elf -w /elf vapix-fedora-cris make all_cris

# Targets run in the cargo-acap image:
all_cargo_acap: arch.AArch64 arch.ARMv5TEJ arch.ARMv6 arch.ARMv7 arch.ARMv7-HF arch.MIPS

arch.AArch64: main.c
	aarch64-linux-gnu-gcc -g0 -o $@ $^

arch.ARMv5TEJ: main.c
	arm-linux-gnueabi-gcc -g0 -march=armv5tej -o $@ $^

arch.ARMv6: main.c
	arm-linux-gnueabi-gcc -g0 -march=armv6 -o $@ $^

arch.ARMv7: main.c
	arm-linux-gnueabi-gcc -g0 -march=armv7 -o $@ $^

arch.ARMv7-HF: main.c
	arm-linux-gnueabihf-gcc -g0 -march=armv7 -o $@ $^

arch.MIPS: main.c
	mipsisa32r2el-axis-linux-gnu-gcc -g0 -o $@ $^

# Targets run in the fedora-cris image:
all_cris: arch.CRISv32

# The M1054 uses an ARTPEC-3 SOC, which is Architecture::Crisv32
# M1054_5_51_7.bin's /bin/busybox contains E_FLAGS=0x02
# gcc -march=v32 produces an executable with the same flags
arch.CRISv32: nostdlib.c
	cris-linux-gnu-gcc -g0 -march=v32 -nostdlib -o $@ $^
