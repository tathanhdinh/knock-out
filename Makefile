obj-m += parrot.o
parrot-objs := src/ffi.o libparrot.a
src := ${PWD}
ldflags-y += --undefined=init_module --undefined=cleanup_module

all modules:
	make -C /lib/modules/$(shell uname -r)/build M=$(PWD) modules V=1

$(obj)/libparrot.a: $(src)/src/lib.rs $(src)/Cargo.toml FORCE
	cd $(src) && cargo rustc --release -- -C relocation-model=static -C code-model=kernel -Z plt=y
	cp $(src)/target/release/libparrot.a $(obj)

clean:
	cargo clean
	make -C /lib/modules/$(shell uname -r)/build M=$(PWD) clean
