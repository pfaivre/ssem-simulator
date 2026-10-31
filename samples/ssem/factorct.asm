; The first program was written by Tom Kilburn. It was a program to find the highest proper factor of any number a;
; this was done by trying every integer b from a-1 downward until one was found that divided exactly into a.
; The necessary divisions were done not by long division but by repeated subtraction of b
; (because the "Baby" only had a hardware subtractor).
;
; https://web.archive.org/web/20081006200609/http://www.computer50.org/mark1/firstprog.html
;
; When stopped, the answer will appear on line 27
; With the number entered in the present file, it should be 131072
;
; Encoding notes:
; - Line 00 is never executed (CI is pre-incremented, so execution starts at
;   line 01); "JMP 0" is just a dummy whose word value is 0.
; - JMP/JRP are indirect: JRP S sets CI = CI + store[S], JMP S sets CI = store[S],
;   and CI is incremented once more before the next fetch. So to land on line T,
;   a JRP on line L needs store[S] = T - L - 1, and a JMP needs store[S] = T - 1.
;   Line 20 is the JRP offset for line 08; line 22 is the JMP word for line 19.
; - LDN negates on load, so the candidate factor b is kept as a pair: line 27 = +b,
;   line 26 = -b.
;
; Data flow: 24 = a (input), 23 = -(a+1), 27 = b (counted down from a), 26 = -b,
; 25 = remainder test cell. Lines 05-08 subtract b from a+1 repeatedly (division
; by repeated subtraction); line 12 stops when the remainder reaches 0.

00 JMP 0       ;
01 LDN 24      ; A = -a
02 STO 26      ;   26 = -a  (i.e. -b, with b = a initially)
03 LDN 26      ;   A = a
04 STO 27      ;   27 = a = b   (candidate factor starts at a)
05 LDN 23      ; outer loop: A = -(-(a+1)) = a+1
06 SUB 27      ;   A = A - b   (subtract b again and again)
07 CMP         ;   skip next if A < 0 (enough b's subtracted)
08 JRP 20      ;   A >= 0 -> back to line 06 (store[20] = -3: 6 = 8 + (-3) + 1)
09 SUB 26      ;   A = A - (-b) = A + b  -> remainder of (a+1) mod b, in (0,b] or 0
10 STO 25      ;   25 = remainder
11 LDN 25      ;   A = -remainder
12 CMP         ;   skip STP if remainder > 0 (b is not a factor yet)
13 STP         ;   remainder == 0 -> b divides: stop, answer is on line 27
14 LDN 26      ; next candidate: A = b
15 SUB 21      ;   A = b - 1   (store[21] = 1)
16 STO 27      ;   27 = b - 1
17 LDN 27      ;   A = -(b-1)
18 STO 26      ;   26 = -(b-1) = -b   (keep the +/-b pair in sync)
19 JMP 22      ;   back to outer loop line 05 (store[22] = 4: JMP lands on 4+1)
20 NUM -3      ; JRP offset for line 08: 6 - 8 - 1
21 NUM 1       ; ONE: constant 1 (used by line 15)
22 NUM 4       ; JMP word for line 19: CI = 4 -> next fetch line 05
23 NUM -262144 ; -(a+1), used by line 05
24 NUM 262143  ; a: the number to find the highest factor of (input)
25 NUM 0       ; remainder test cell (lines 10-12)
26 NUM 0       ; -b: negated candidate factor
27 NUM 0       ; b: candidate factor; holds the answer when stopped
28 NUM 0       ;
29 NUM 0       ;
30 NUM 0       ;
31 NUM 0       ;
