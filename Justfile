@build *ARGS:
	cargo build {{ARGS}}

@debug: build
	ugdb --gdb=rust-gdb target/debug/ssg
