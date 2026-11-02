; This programme is written for the Small-Scale Experimental Machine (SSEM)
; It computes a given iteration of the Fibonacci sequence (specified in address 29)
;
; By David Tarnoff "Programming the Manchester Baby - Part 2" https://www.youtube.com/watch?v=y8jTDTe9yrg
;
; When stopped, line 27 holds the target Fibonacci number
; (with line 29 = 46, line 27 = F(46) = 1836311903).
;
; Encoding notes:
; - Line 00 is never executed (CI is pre-incremented, so execution starts at
;   line 01); here it doubles as the constant ONE = 1 used by line 02.
; - JMP is indirect: JMP S sets CI = store[S], then CI is incremented before the
;   next fetch. Line 18 jumps via line 30 (value 0), so CI = 0 and the next fetch
;   is line 01: the main loop is lines 01-18.
; - LDN negates on load, so values are flipped through the accumulator when a
;   positive copy is needed (lines 01-05 and 12-15).
;
; Data flow: 31 = loop counter, 27 = F(n), 28 = F(n-1), 26 = -F(n+1), 29 = target.
; Each iteration increments the counter, computes F(n+1) = F(n) + F(n-1), and
; shifts the window (28 <- 27, 27 <- 26).

00 NUM 1    ; ONE = 1 (constant, used by line 02); line 00 is never executed
01 LDN 31   ; loop: A = -counter
02 SUB 0    ;   A = -counter - 1 = -(counter + 1)
03 STO 31   ;   31 = -(counter + 1)
04 LDN 31   ;   A = counter + 1
05 STO 31   ;   counter = counter + 1
06 SUB 29   ;   A = counter - target
07 CMP      ;   skip STP while counter < target
08 STP      ;   counter >= target: stop, F(target) is on line 27
09 LDN 27   ;   A = -F(n)
10 SUB 28   ;   A = -F(n) - F(n-1) = -F(n+1)
11 STO 26   ;   26 = -F(n+1)
12 LDN 27   ;   A = -F(n)
13 STO 28   ;   28 = -F(n)   (new F(n-1), negated)
14 LDN 28   ;   A = F(n)
15 STO 28   ;   28 = F(n)    (new F(n-1), stored positive)
16 LDN 26   ;   A = F(n+1)
17 STO 27   ;   27 = F(n+1)  (new F(n))
18 JMP 30   ;   back to loop line 01 (store[30] = 0 -> CI = 0 -> fetch line 01)
19 JMP 0    ;
20 JMP 0    ;
21 JMP 0    ;
22 JMP 0    ;
23 JMP 0    ;
24 JMP 0    ;
25 JMP 0    ;
26 NUM 0    ; -F(n+1): temp holding the negated new element (line 11)
27 NUM 1    ; F(n), starts at F(1) = 1; holds F(target) when stopped
28 NUM 0    ; F(n-1), starts at F(0) = 0
29 NUM 46   ; target index (input)
30 NUM 0    ; JMP word for line 18: CI = 0 -> next fetch line 01
31 NUM 0    ; loop counter, starts at 0
