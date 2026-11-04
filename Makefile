.PHONY: help install

help:
	@printf '%s\n' 'make install  Build the current source and install a local Arch Linux package.'

install:
	@bash packaging/arch/install.sh
