# Small-Scale Experimental Machine (SSEM) simulator

The SSEM, also known as the Manchester Baby was the first electronic stored-program computer.

As it is very simple, it is a good subject to study the basic principles of computing.

This program aims at simulating accurately the SSEM while allowing to play with it and tweak it.

Checkout [manchester-baby-sim](https://github.com/pfaivre/manchester-baby-sim) for an interactive Python implementation.

# Test it

```sh
cargo run -r factorct.asm
```

# Roadmap

- [x] Read assembler files
- [x] Run the program
- [ ] Assembler language validation
- [ ] Read binary representation files (.snp)
- [ ] Improve readability (display option)
- [ ] Unit and functional tests
- [ ] Implement breakpoints: automatically stop at a given condition

# Documentation

Pending...

# Bibliography

David Tarnoff, "Programming the 1948 Manchester Baby (SSEM)" https://www.youtube.com/watch?v=o7ozlF5ujUw

Chris P Burton, "The Manchester University Small-Scale Experimental Machine Programmer's Reference Manual" http://curation.cs.manchester.ac.uk/computer50/www.computer50.org/mark1/prog98/ssemref.html

Computer Conservation Society, "SSEM - Technical Overview" https://computerconservationsociety.org/ssemvolunteers/volunteers/introframe.html

David Sharp, "Manchester Baby Simulator" https://davidsharp.com/baby/

Brian Napper, "The Manchester Small Scale Experimental Machine -- "The Baby""
https://web.archive.org/web/20081013180637/http://www.computer50.org/mark1/new.baby.html#specification

# License

This program is licensed under the MIT license.
