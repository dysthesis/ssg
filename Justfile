@mutants *ARGS:
	cargo mutants -j8 -- {{ARGS}}

@build *ARGS:
	cargo build {{ARGS}}

@debug: build
	ugdb --gdb=rust-gdb target/debug/ssg

@test *ARGS:
	cargo llvm-cov nextest \
		--html \
		--release \
		--branch \
		-- {{ARGS}}
