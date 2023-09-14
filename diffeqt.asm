; diffeqt.asm - Calculate points on a parabola using a difference
; equation, and draw the parabola. By Magnus Olsson, Sweden.

00 NUM 0
01 LDN 29
02 SUB 29
03 STO 29
04 CMP
05 STP
06 LDN 29
07 STO 29
08 LDN 22
09 SUB 29
10 STO 30
11 LDN 30
12 STO 22
13 LDN 8
14 SUB 28
15 STO 30
16 LDN 30
17 STO 8
18 SUB 27
19 STO 12
20 LDN 28
21 SUB 26
22 STO 28
23 LDN 28
24 STO 28
25 JMP 31
26 NUM 1
27 NUM -8192
28 NUM -6
29 NUM 131072
30 NUM 0
31 NUM 0
