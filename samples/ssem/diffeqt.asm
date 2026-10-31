; diffeqt.asm - Calculate points on a parabola using a difference
; equation, and draw the parabola. By Magnus Olsson, Sweden.
;
; Encoding notes:
; - Line 00 is never executed (CI is pre-incremented, so execution starts at
;   line 01); here it is just a zero word.
; - JMP/JRP are indirect: JMP S sets CI = store[S]. Line 25 jumps via line 31,
;   whose value 0 makes CI = 0, so the next fetch is line 01: the main loop is
;   lines 01-25. (For a JMP landing on line T, the jump word holds T - 1.)
; - LDN negates on load; to negate the accumulator back, store it in a temp
;   line and LDN that (lines 10-11, 15-16, 23-24 use temp line 30).
;
; Data flow: line 29 is the position accumulator, line 22 the first-difference
; accumulator, line 8 the second-difference accumulator, line 28 a step counter.

00 NUM 0      ;
01 LDN 29     ; loop: A = -pos
02 SUB 29     ;   A = -pos - pos = -2*pos
03 STO 29     ;   pos = -2*pos
04 CMP        ;   skip STP while A < 0
05 STP        ;   halt when -2*pos >= 0
06 LDN 29     ;   A = -pos
07 STO 29     ;   pos = -pos (so over a full loop pos doubles)
08 LDN 22     ;   A = -vel
09 SUB 29     ;   A = -vel - pos
10 STO 30     ;   temp = -vel - pos
11 LDN 30     ;   A = vel + pos
12 STO 22     ;   vel = vel + pos   (first difference accumulates position)
13 LDN 8      ;   A = -acc
14 SUB 28     ;   A = -acc - step
15 STO 30     ;   temp = -acc - step
16 LDN 30     ;   A = acc + step
17 STO 8      ;   acc = acc + step   (second difference accumulates step)
18 SUB 27     ;   A = A - (-8192) = A + 8192
19 STO 12     ;   line 12 = A (note: line 12 is also executed as code)
20 LDN 28     ;   A = -step
21 SUB 26     ;   A = -step - 1
22 STO 28     ;   step = -step - 1
23 LDN 28     ;   A = step + 1
24 STO 28     ;   step = step + 1   (step counter, starts at -6)
25 JMP 31     ;   jump via line 31 (value 0) -> CI = 0 -> next fetch line 01
26 NUM 1      ; ONE: constant 1 (used by line 21)
27 NUM -8192  ; constant -8192 (used by line 18)
28 NUM -6     ; step counter init (used by lines 14, 20-24)
29 NUM 131072 ; position accumulator init (used by lines 1-3, 6-7, 9)
30 NUM 0      ; temp for negating the accumulator
31 NUM 0      ; jump word for line 25: JMP 31 -> CI = 0 -> loop to line 01
