; Machine state saved by ssem-simulator v1.1.0
; CI: 0  A: 0  cycles: 0
00 JMP 00
01 LDN 29; loop: A = -pos
02 SUB 29;   A = -pos - pos = -2*pos
03 STO 29;   pos = -2*pos
04 CMP 00;   skip STP while A < 0
05 STP 00;   halt when -2*pos >= 0
06 LDN 29;   A = -pos
07 STO 29;   pos = -pos (so over a full loop pos doubles)
08 LDN 22
09 SUB 29
10 STO 30;   temp = -vel - pos
11 LDN 30;   A = vel + pos
12 STO 22;   vel = vel + pos   (first difference accumulates position)
13 LDN 08;   A = -acc
14 SUB 28;   A = -acc - step
15 STO 30;   temp = -acc - step
16 LDN 30;   A = acc + step
17 STO 08;   acc = acc + step   (second difference accumulates step)
18 SUB 27;   A = A - (-8192) = A + 8192
19 STO 12;   line 12 = A (note: line 12 is also executed as code)
20 LDN 28;   A = -step
21 SUB 26;   A = -step - 1
22 STO 28;   step = -step - 1
23 LDN 28;   A = step + 1
24 STO 28;   step = step + 1   (step counter, starts at -6)
25 JMP 31;   jump via line 31 (value 0) -> CI = 0 -> next fetch line 01
26 JMP 01; ONE: constant 1 (used by line 21)
27 NUM -8192; constant -8192 (used by line 18)
28 NUM -6; step counter init (used by lines 14, 20-24)
29 NUM 131072; position accumulator init (used by lines 1-3, 6-7, 9)
30 JMP 00; temp for negating the accumulator
31 JMP 00;foobar
