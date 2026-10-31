; virpet.asm - Virtual pet, like the "Tamagotchi" children's toy The
; virtual pet must constantly be fed.  As long as it has sufficient
; food, it shows a happy face, and a full belly.  When it runs out, it
; shows a sad face and a slim profile.  You can then pause the machine
; to give it more food by loading a number, e.g. 512, into line 22.  It
; will then return to a happy face until it runs out again. By Achut
; Reddy, USA.
;
; Encoding notes:
; - Line 00 is never executed (CI is pre-incremented, so execution starts at
;   line 01); here it doubles as the zero constant read by line 12, and its
;   value 0 is also the JMP target of lines 07 and 21 (JMP 0 -> CI = 0 ->
;   next fetch line 01, i.e. loop).
; - JMP/JRP are indirect: JMP S sets CI = store[S], JRP S sets CI = CI + store[S],
;   with CI incremented once more before the next fetch. Line 26 holds the JRP
;   offset for line 16: 14 = 16 + (-3) + 1.
; - LDN negates on load; line 25 is the temp used to flip the sign of the food
;   counter (lines 01-05).
;
; Data flow: 22 = food level (starts at 1000, one bite per cycle; refill it in
; the TUI to feed the pet), 25 = temp, 30/31 = the two face bitmaps shown on the
; store tube, 23/24 = their negated constants, 27 = -1 (the decrement),
; 26 = JRP offset, 28/29 = belly/profile display rows.

00 NUM 0         ; zero word: never executed; read as 0 by line 12, and the JMP-0 loop target
01 LDN 22        ; loop: A = -food
02 SUB 27        ;   A = -food - (-1) = -(food - 1)   (line 27 holds -1)
03 STO 25        ;   25 = -(food - 1)
04 LDN 25        ;   A = food - 1
05 STO 22        ;   food = food - 1   (one bite per cycle)
06 CMP           ;   skip line 07 when food has run out (A = food - 1 < 0)
07 JMP 0         ;   still fed -> loop (store[0] = 0 -> CI = 0 -> fetch line 01)
08 LDN 23        ; hungry: A = -store[23] = 35651904 (sad face bitmap)
09 STO 31        ;   31 = sad face
10 LDN 24        ;   A = -store[24] = 29360256 (slim profile bitmap)
11 STO 30        ;   30 = slim profile
12 LDN 0         ;   A = -store[0] = 0
13 STO 22        ;   food = 0
14 LDN 22        ; wait-for-food spin: A = -food
15 CMP           ;   skip line 16 if food > 0 (pet has been refilled)
16 JRP 26        ;   food == 0 -> spin back to line 14 (store[26] = -3: 14 = 16 + (-3) + 1)
17 LDN 23        ; refilled: A = 35651904
18 STO 30        ;   30 = happy face
19 LDN 24        ;   A = 29360256
20 STO 31        ;   31 = full belly
21 JMP 0         ;   loop (store[0] = 0 -> fetch line 01)
22 NUM 1000      ; food level (input; change this value to refill)
23 NUM -35651904 ; negated face bitmap (read by lines 08, 17)
24 NUM -29360256 ; negated profile bitmap (read by lines 10, 19)
25 NUM 0         ; temp for the food-counter sign flip (lines 03-04)
26 NUM -3        ; JRP offset for line 16: 14 - 16 - 1
27 NUM -1        ; constant -1 (food decrement, line 02)
28 NUM 20971648  ; display row (head and eyes)
29 NUM 448       ; display row (arms)
30 NUM 35651904  ; face bitmap (written by lines 11, 18)
31 NUM 29360256  ; face bitmap (written by lines 09, 20)
