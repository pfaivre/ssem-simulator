; Written by: 	Bas Wijnen of Holland.
; Obtained from: 	The 1998 SSEM Programming Competition.
; Description: 	Each time the program is run (pressing the EXEC button) the next prime number in the sequence is generated.
; Parameter Lines: 	-
; Result Lines: 	21: generated primes
;
; Source: https://madrona.ca/e/SSEM/programs/primes.html

00 JMP 24    ; wraparound entry from line 30 -> line 08; word value 24 = jump data for lines 20 and 26
01 LDN 21    ; A = -n
02 STO 21    ;   n = -n
03 LDN 21    ;   A = n
04 SUB 15    ;   A = n + 1
05 STO 21    ;   n = n + 1  (next candidate; the first tested is 2)
06 LDN 15    ; A = 1  (line 15 holds -1)
07 STO 22    ;   d = 1
08 LDN 22    ; d++: A = -d   <- line 00 lands here
09 STO 22    ;   d = -d
10 LDN 22    ;   A = d
11 SUB 15    ;   A = d + 1
12 STO 22    ;   d = d + 1  (reached from line 06 this resets d to 2)
13 SUB 21    ; A = d - n
14 CMP       ;   skip the halt if d < n (more divisors to try)
15 NUM -1    ; halt: no divisor below n, so n is prime (word -1 decodes as STP); also the constant -1 for the SUB 15 above
16 LDN 21    ; A = -n
17 STO 23    ;   r = -n
18 LDN 23    ;   A = n
19 SUB 22    ; sub loop: A = A - d
20 JMP 0     ;   via word 00 (= 24) -> line 25; word value 0 is the jump word for line 31
21 NUM 1     ; <--- n: seed (input) and ---> the prime when the machine stops
22 NUM 0     ; d: trial divisor
23 NUM 0     ; r: remainder cell
24 NUM 7     ; jump word for line 00 (JMP): 8 - 1
25 CMP       ;   A < 0: subtraction done -> line 27
26 JRP 0     ;   A >= 0: subtract d again (word 00 = 24; 19 - 26 - 1 = -8 = 24 mod 32)
27 STO 23    ;   r = n - k*d  (negative)
28 LDN 23    ; A = k*d - n
29 SUB 22    ;   A = (k-1)*d - n = -(n mod d)
30 CMP       ;   skip line 31 if d does not divide n (wraps to line 00: d += 1)
31 JMP 20    ;   d divides n: composite; via word 20 (= 0) -> line 01, try n + 1
