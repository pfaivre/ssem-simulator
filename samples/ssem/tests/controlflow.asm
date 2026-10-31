; controlflow.asm - a demo program with a deliberately dense control-flow graph.
;
; Press F in the TUI until the top bar shows [ FLOW: CONTROL ] to see the
; static graph between the store lines: backward edges (loops, incl. the
; 31 -> 0 store wrap) in bold red, forward jumps in yellow, CMP skips in
; white, the STP halt marker as a red block, and enough overlapping edges
; to overflow the four gutter lanes (shown as "⇢NN" labels).
;
; The program itself is a countdown: line 02 adds 4 to the accumulator on
; each pass (SUB of NUM -4) and line 03 skips while it is negative; once it
; turns non-negative, control falls through to the STP on line 07.

00 NUM 0    ; executed after the wrap: JMP 0 -> line 01
01 LDN 26   ; A = -30 (the countdown value lives in line 26)
02 SUB 27   ; A += 4 each pass (store[27] = NUM -4)
03 CMP      ; while A < 0, skip to line 05
04 JMP 30   ; store[30] = 5 -> lands on line 06
05 JRP 27   ; 5 - 4 + 1 = 2: loop back to the counter
06 CMP      ; skip to line 08
07 STP      ; halt (red block in the graph)
08 JMP 28   ; store[28] = 8 -> lands on line 09
09 CMP      ; skip to line 11
10 JRP 29   ; 10 - 9 + 1 = 2: another loop back
11 LDN 26
12 STO 26
13 LDN 26
14 SUB 27
15 STO 26
16 LDN 26
17 CMP      ; skip to line 19
18 JMP 30   ; -> line 06 (backward edge: red loop)
19 LDN 26
20 STO 26
21 LDN 26
22 SUB 27
23 STO 26
24 LDN 26
25 JMP 26   ; store[26] = 30 -> lands on line 31 (long forward jump)
26 NUM 30   ; jump target for line 25 (and the countdown value)
27 NUM -4  ; JRP offset for line 05
28 NUM 8   ; jump target for line 08
29 NUM -9  ; JRP offset for line 10
30 NUM 5   ; jump target for lines 04 and 18
31 LDN 26   ; falls through the end of the store: wraps 31 -> 0 (red)
