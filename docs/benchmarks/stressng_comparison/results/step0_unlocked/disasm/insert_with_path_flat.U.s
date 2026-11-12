push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x148,%rsp
mov    %r8,%r12
mov    %ecx,%ebp
mov    %rdx,0x8(%rsp)
mov    %rsi,%r14
mov    %rdi,%r13
lea    0x50(%r8),%rbx
lea    0x60(%rsp),%rax
mov    %rax,0xc0(%rsp)
xor    %r11d,%r11d
mov    %r8,0x28(%rsp)
mov    %rbx,0x40(%rsp)
mov    %rdi,0x18(%rsp)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
nopl   (%rax)
mov    %rbp,0x30(%rsp)
movzbl 0xf(%r14),%ebp
cmp    $0xc,%ebp
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xeb4>
lea    -0x29d42a(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8bad>
movslq (%rcx,%rbp,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    0x30(%rsp),%r9
cmp    $0x8,%r9b
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2b42>
addq   $0x40,0x200(%r13)
incq   0x208(%r13)
incq   0x218(%r13)
mov    0x18(%r13),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x11c5>
mov    (%rbx),%rax
mov    %rax,0x18(%r13)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1270>
mov    %r11,0x38(%rsp)
lea    0x8(%r14),%rdx
mov    %r14,0x20(%rsp)
mov    0x8(%r14),%r14
movzbl %r14b,%ebp
lea    0x1(%rbp),%r8
mov    0x30(%rsp),%rax
cmp    $0x1,%al
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x24c8>
dec    %eax
movzbl %al,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6194>
lea    -0x2997c6(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
shl    $0x8,%rax
mov    %r14,%rcx
xor    0x8(%rsp),%rcx
test   %rcx,%rax
mov    0x30(%rsp),%rsi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x24c8>
mov    %rbx,%r15
mov    (%rdx),%eax
mov    0x3(%rdx),%ecx
mov    %ecx,0x5b(%rsp)
mov    %eax,0x58(%rsp)
movzbl %sil,%eax
lea    0x1(%rsi),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
mov    0x38(%rsp),%r9
mov    0x20(%rsp),%r14
mov    0x18(%rsp),%r13
mov    %rsi,%rbp
mov    0x8(%rsp),%r10
cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60a4>
mov    %r10,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x170>
movzbl %dl,%edx
mov    %r13,%rdi
mov    %r14,%rsi
mov    %eax,%ecx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x11b2>
mov    (%r14),%rbp
movzbl 0x74(%rbp),%eax
mov    %rax,0x10(%rsp)
movzbl %al,%edi
mov    0x30(%rsp),%rax
sub    %dil,%al
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0xd33>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61ec>
lea    -0x299888(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x8(%r14),%r8
mov    %r8,%rcx
xor    0x8(%rsp),%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xd33>
cmpb   $0x7,0x10(%rsp)
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6224>
lea    -0x2998bd(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    (%rbx),%rax
test   %rax,%rax
mov    0x30(%rsp),%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x414>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f3>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f3>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x271>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29990f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f3>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2b1>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29994f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f3>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f1>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29998f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f3>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x331>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2999cf(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f3>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x371>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299a0f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f3>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3ad>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299a4b(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f3>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3e9>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299a87(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %rbx,%r15
mov    %r11,%rbx
lea    0x8(%r14),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0x5b(%rsp)
mov    %ecx,0x58(%rsp)
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60a4>
mov    0x8(%rsp),%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x450>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0xd23>
mov    %r14,0x20(%rsp)
mov    (%r14),%r14
movzbl 0x5(%r14),%ebp
mov    0x30(%rsp),%rax
sub    %bpl,%al
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0xded>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61ec>
movzbl %bpl,%edi
lea    -0x299b61(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x20(%rsp),%rcx
mov    0x8(%rcx),%r8
mov    %r8,%rcx
xor    0x8(%rsp),%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xded>
cmp    $0x7,%bpl
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6224>
lea    -0x299b9a(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    (%rbx),%rax
test   %rax,%rax
mov    0x20(%rsp),%r14
mov    0x30(%rsp),%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6f6>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6d5>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6d5>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x553>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299bf1(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6d5>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x593>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299c31(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6d5>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5d3>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299c71(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6d5>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x613>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299cb1(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6d5>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x653>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299cf1(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6d5>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x68f>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299d2d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6d5>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6cb>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299d69(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %rbx,%r15
mov    %r11,%rbx
lea    0x8(%r14),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0x5b(%rsp)
mov    %ecx,0x58(%rsp)
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60a4>
mov    0x8(%rsp),%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x730>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0xd23>
cmp    $0x8,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6236>
mov    (%r14),%rax
mov    %r11,%rcx
shl    $0x4,%rcx
mov    %r14,0xc8(%rsp,%rcx,1)
mov    0x30(%rsp),%rbp
mov    %bpl,0xd0(%rsp,%rcx,1)
dec    %bpl
lea    0x0(,%rbp,8),%ecx
mov    0x8(%rsp),%rdx
shr    %cl,%rdx
movzbl %dl,%ecx
inc    %r11
shl    $0x4,%ecx
lea    (%rax,%rcx,1),%r14
add    $0x40,%r14
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    (%r14),%rdx
movzbl 0x5(%rdx),%esi
mov    0x30(%rsp),%rbp
mov    %ebp,%eax
sub    %sil,%al
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0xe4e>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61ec>
movzbl %sil,%edi
lea    -0x299e8e(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x8(%r14),%r8
mov    %r8,%rcx
xor    0x8(%rsp),%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xe4e>
cmp    $0x7,%sil
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6224>
lea    -0x299ec2(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xa14>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9f3>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9f3>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x871>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299f0f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9f3>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x8b1>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299f4f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9f3>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x8f1>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299f8f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9f3>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x931>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x299fcf(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9f3>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x971>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a00f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9f3>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9ad>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a04b(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9f3>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9e9>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a087(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %rbx,%r15
mov    %r11,%rbx
lea    0x8(%r14),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0x5b(%rsp)
mov    %ecx,0x58(%rsp)
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60a4>
mov    0x8(%rsp),%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xa50>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0xd23>
mov    0x30(%rsp),%rbp
cmp    $0x1,%bpl
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2537>
lea    -0x1(%rbp),%eax
movzbl %al,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6194>
lea    -0x29a156(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rcx
shl    $0x8,%rcx
mov    0x8(%r14),%rax
mov    %rax,%rdx
xor    0x8(%rsp),%rdx
test   %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2537>
mov    (%rbx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xcc1>
mov    0x68(%r12),%rdx
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xca0>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xca0>
movzbl 0x49(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xb1e>
mov    0x8(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x29a1bc(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x2,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xca0>
movzbl 0x4a(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xb5e>
mov    0x10(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x29a1fc(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x3,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xca0>
movzbl 0x4b(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xb9e>
mov    0x18(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x29a23c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x4,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xca0>
movzbl 0x4c(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xbde>
mov    0x20(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x29a27c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x5,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xca0>
movzbl 0x4d(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc1e>
mov    0x28(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x29a2bc(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x6,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xca0>
movzbl 0x4e(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc5a>
mov    0x30(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x29a2f8(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x7,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xca0>
movzbl 0x4f(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc96>
mov    0x38(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x29a334(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x8,%rcx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %rbx,%r15
mov    %r11,%rbx
lea    0x8(%r14),%rcx
movzbl %al,%r8d
inc    %r8
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x5b(%rsp)
mov    %eax,0x58(%rsp)
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60a4>
mov    0x8(%rsp),%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xd00>
movzbl %dl,%edx
mov    %r13,%rdi
mov    %r14,%rsi
mov    %eax,%ecx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x11b5>
mov    %r14,0x20(%rsp)
mov    0x10(%rsp),%rax
lea    -0x1(%rax),%esi
lea    0x0(,%rsi,8),%ecx
mov    0x8(%rsp),%rbx
shr    %cl,%rbx
mov    %ebx,%r14d
shr    $0x5,%r14d
and    $0x7,%r14d
mov    0x0(%rbp,%r14,4),%eax
mov    $0x1,%edx
mov    %ebx,%ecx
shl    %cl,%edx
bt     %ebx,%eax
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x14a7>
cmp    $0x8,%r11
mov    0x40(%rsp),%rbx
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x624b>
dec    %edx
and    %edx,%eax
mov    %eax,%ecx
shr    $1,%ecx
and    $0x55555555,%ecx
sub    %ecx,%eax
mov    %eax,%ecx
and    $0x33333333,%ecx
shr    $0x2,%eax
and    $0x33333333,%eax
add    %ecx,%eax
mov    %eax,%ecx
shr    $0x4,%ecx
add    %eax,%ecx
and    $0xf0f0f0f,%ecx
imul   $0x1010101,%ecx,%eax
mov    %r11,%rcx
shl    $0x4,%rcx
shr    $0x14,%eax
and    $0xfffffff0,%eax
add    0x20(%rbp,%r14,8),%rax
mov    0x20(%rsp),%rdx
mov    %rdx,0xc8(%rsp,%rcx,1)
mov    0x10(%rsp),%rdx
mov    %dl,0xd0(%rsp,%rcx,1)
inc    %r11
mov    %esi,%ebp
mov    %rax,%r14
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    %r11,0x38(%rsp)
lea    -0x1(%rbp),%r15d
lea    0x0(,%r15,8),%ecx
mov    0x8(%rsp),%rdx
shr    %cl,%rdx
mov    %edx,%ecx
and    $0xf,%cl
mov    $0x1,%r12d
shl    %cl,%r12d
mov    %edx,%eax
and    $0xf,%eax
movzwl 0x6(%r14),%ecx
bt     %eax,%ecx
movzbl 0x4(%r14),%eax
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1804>
cmp    $0x4,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x17c1>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1b11>
cmp    %dl,0x8(%r14)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1b11>
xor    %ecx,%ecx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1b28>
lea    -0x1(%rsi),%edi
lea    0x0(,%rdi,8),%ecx
mov    0x8(%rsp),%rax
shr    %cl,%rax
movzbl 0x4(%rdx),%ecx
test   %ecx,%ecx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2173>
movzbl 0x8(%rdx),%r8d
cmp    %al,%r8b
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a42>
xor    %r9d,%r9d
cmp    $0x7,%r11
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x62b1>
mov    %r11,%rax
shl    $0x4,%rax
mov    %r14,0xc8(%rsp,%rax,1)
mov    %sil,0xd0(%rsp,%rax,1)
inc    %r11
shl    $0x4,%r9d
lea    (%rdx,%r9,1),%r14
add    $0x10,%r14
mov    %edi,%ebp
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
cmp    $0x7f,%ebp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6c>
lea    -0x6(%rbp),%eax
cmp    $0x6,%al
mov    %r11,0x38(%rsp)
mov    %r14,0x20(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x27ba>
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10d8>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10b7>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10b7>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xf35>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a5d3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10b7>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xf75>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a613(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10b7>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xfb5>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a653(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10b7>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xff5>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a693(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10b7>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1035>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a6d3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10b7>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1071>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a70f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10b7>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10ad>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a74b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
lea    -0x4(%rbp),%eax
lea    0x8(%r14),%r13
movzbl %al,%ebx
mov    0x8(%r14),%rax
lea    -0x29a7a5(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rbx,8),%rsi
mov    %rsi,%r15
and    %rax,%r15
lea    0x1(%r15),%r11
mov    0x30(%rsp),%rcx
cmp    %bl,%cl
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2dfb>
sub    %bl,%cl
movzbl %cl,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6194>
lea    -0x29a7d6(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rdi,8),%rdx
lea    0x0(,%rbx,8),%r14d
mov    %r14d,%ecx
shl    %cl,%rdx
xor    0x8(%rsp),%rax
test   %rax,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e03>
mov    0x0(%r13),%eax
mov    0x3(%r13),%ecx
mov    %ecx,0x5b(%rsp)
mov    %eax,0x58(%rsp)
mov    0x30(%rsp),%rbp
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
mov    0x40(%rsp),%r10
mov    0x38(%rsp),%r9
mov    0x20(%rsp),%r14
mov    0x18(%rsp),%r13
mov    0x8(%rsp),%r8
nop
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60a4>
mov    %r8,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1180>
movzbl %dl,%edx
mov    %r13,%rdi
mov    %r14,%rsi
mov    %eax,%ecx
mov    %r11,%r8
mov    %r10,%r15
mov    %r9,%rbx
call   ADDR <expanse_trie::mutate::wrap_skip_level::<false>>
mov    %rbx,%r11
mov    %r15,%rbx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    %r11,%rbp
movq   $0x0,0x58(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x58(%rsp),%rdi
call   *0xcbdca(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61a6>
mov    0x58(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x61a6>
mov    $0x1000,%edx
mov    %rbx,%rdi
xor    %esi,%esi
call   *0xcbdb4(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x220(%r13),%rax
mov    %rax,(%rbx)
movq   $0x1,0x8(%rbx)
mov    %rbx,0x220(%r13)
lea    0xfc0(%rbx),%rax
mov    $0x41,%ecx
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    0x18(%r13),%rdx
mov    %rdx,(%rax)
mov    %rax,0x18(%r13)
lea    -0x40(%rax),%rdx
mov    0x18(%r13),%rsi
mov    %rsi,-0x40(%rax)
mov    %rdx,0x18(%r13)
add    $0xffffffffffffff80,%rax
add    $0xfffffffffffffffe,%rcx
cmp    $0x3,%rcx
jg     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1240>
add    $0x40,%rbx
mov    %rbp,%r11
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movdqu %xmm0,0x30(%rbx)
movdqu %xmm0,0x20(%rbx)
movdqu %xmm0,0x10(%rbx)
movb   $0x8,0x5(%rbx)
movl   $0x0,0x5b(%rsp)
movl   $0x0,0x58(%rsp)
mov    %rbx,(%r14)
mov    0x58(%rsp),%eax
mov    0x5b(%rsp),%ecx
mov    %eax,0x8(%r14)
mov    %ecx,0xb(%r14)
movb   $0x1,0xf(%r14)
mov    0x40(%rsp),%rbx
mov    (%rbx),%rax
mov    $0x8,%bpl
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x50>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x50>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x131e>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a9bc(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x50>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x135e>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29a9fc(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x50>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x139e>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29aa3c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x50>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x13de>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29aa7c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x50>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x141e>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29aabc(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x50>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x145e>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29aafc(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x50>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29ab40(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x46>
movdqa 0x0(%rbp),%xmm0
movdqa 0x10(%rbp),%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x1,%xmm2
movdqa -0x2bf4d2(%rip),%xmm3        # ADDR <anon.ADDR.407.llvm.ADDR+0x10>
pand   %xmm3,%xmm2
psubb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
movdqa -0x2bf236(%rip),%xmm4        # ADDR <anon.ADDR.407.llvm.ADDR+0x2c0>
pand   %xmm4,%xmm2
psrlw  $0x2,%xmm1
pand   %xmm4,%xmm1
paddb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x4,%xmm2
paddb  %xmm1,%xmm2
movdqa -0x2bf5ac(%rip),%xmm5        # ADDR <anon.ADDR.283.llvm.ADDR+0xf6>
pand   %xmm5,%xmm2
movdqa %xmm0,%xmm1
psrlw  $0x1,%xmm1
pand   %xmm3,%xmm1
psubb  %xmm1,%xmm0
movdqa %xmm0,%xmm1
pand   %xmm4,%xmm1
psrlw  $0x2,%xmm0
pand   %xmm4,%xmm0
paddb  %xmm1,%xmm0
movdqa %xmm0,%xmm1
psrlw  $0x4,%xmm1
paddb  %xmm0,%xmm1
pand   %xmm5,%xmm1
paddb  %xmm2,%xmm1
pxor   %xmm0,%xmm0
psadbw %xmm0,%xmm1
pshufd $0xee,%xmm1,%xmm0
paddq  %xmm1,%xmm0
movd   %xmm0,%ecx
cmp    $0xbf,%ecx
mov    %r11,0x38(%rsp)
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a84>
mov    0x30(%rsp),%rbp
cmp    0x10(%rsp),%bpl
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1e06>
cmpb   $0x8,0x10(%rsp)
mov    0x40(%rsp),%rbx
mov    0x20(%rsp),%r14
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6224>
lea    -0x29ac45(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%r8
and    0x8(%r14),%r8
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x179b>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x177a>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x177a>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x15f8>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29ac96(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x177a>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1638>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29acd6(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x177a>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1678>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29ad16(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x177a>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16b8>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29ad56(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x177a>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16f8>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29ad96(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x177a>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1734>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29add2(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x177a>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1770>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29ae0e(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
inc    %r8
mov    0x10(%rsp),%rax
inc    %al
movzbl %bpl,%ecx
movzbl %al,%edx
mov    %r13,%rdi
mov    %r14,%rsi
call   ADDR <expanse_trie::mutate::wrap_skip_level::<false>>
mov    0x38(%rsp),%r11
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
cmp    $0x8,%al
mov    $0x8,%ecx
cmovb  %eax,%ecx
movzbl %dl,%esi
movd   %esi,%xmm0
punpcklbw %xmm0,%xmm0
pshuflw $0x0,%xmm0,%xmm0
pshufd $0x44,%xmm0,%xmm0
movq   0x8(%r14),%xmm1
mov    $0xffffffff,%esi
shl    %cl,%esi
pcmpeqb %xmm0,%xmm1
pmovmskb %xmm1,%ecx
not    %esi
and    %ecx,%esi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1804>
tzcnt  %esi,%ecx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1b28>
cmp    $0x7,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2075>
mov    (%rbx),%rax
test   %rax,%rax
mov    0x28(%rsp),%r12
mov    0x38(%rsp),%r9
mov    0x20(%rsp),%r14
mov    0x18(%rsp),%r13
mov    0x30(%rsp),%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a2c>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a0b>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a0b>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1889>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29af27(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a0b>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x18c9>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29af67(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a0b>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1909>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29afa7(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a0b>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1949>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29afe7(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a0b>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1989>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b027(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a0b>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x19c5>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b063(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a0b>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a01>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b09f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %r13,%rdi
mov    %r14,%rsi
mov    %rbx,%r15
mov    %r9,%rbx
call   ADDR <expanse_trie::mutate::upgrade_l7_to_b::<false>>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x11ba>
cmp    $0x1,%ecx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1b7b>
cmp    %al,%r8b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1dcb>
lea    0x10(%rdx),%r9
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2165>
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2173>
movzbl 0x9(%rdx),%r10d
mov    %r10b,0xa(%rdx)
mov    %r8b,0x9(%rdx)
movups 0x20(%rdx),%xmm0
movups %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2169>
movzwl 0x60(%rbp,%r14,2),%r13d
dec    %edx
and    %edx,%eax
mov    %eax,%ecx
shr    $1,%ecx
and    $0x55555555,%ecx
sub    %ecx,%eax
mov    %eax,%ecx
and    $0x33333333,%ecx
shr    $0x2,%eax
and    $0x33333333,%eax
add    %ecx,%eax
mov    %eax,%ecx
shr    $0x4,%ecx
add    %eax,%ecx
and    $0xf0f0f0f,%ecx
imul   $0x1010101,%ecx,%eax
shr    $0x18,%eax
mov    %rax,0x30(%rsp)
test   %r13,%r13
mov    %rsi,0x70(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x20f2>
mov    $0x2,%eax
cmp    $0x1,%r13d
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x20f7>
cmp    $0x10,%r13w
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x21d4>
lea    0x4(%r13),%rax
cmp    $0x2,%r13w
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1b09>
lea    0x3(%r13),%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2484>
and    $0x1c,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x20f7>
cmp    $0x2,%al
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x202b>
mov    $0x1,%ecx
cmp    %dl,0x9(%r14)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x202b>
mov    0x38(%rsp),%r11
cmp    $0x8,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6285>
mov    %r11,%rax
shl    $0x4,%rax
mov    0x20(%rsp),%rdx
mov    %rdx,0xc8(%rsp,%rax,1)
mov    %bpl,0xd0(%rsp,%rax,1)
cmp    $0x7,%ecx
mov    0x28(%rsp),%r12
mov    0x18(%rsp),%r13
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x629a>
inc    %r11
shl    $0x4,%ecx
add    %rcx,%r14
add    $0x10,%r14
mov    %r15d,%ebp
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    $0x1,%r9d
cmp    %al,0x9(%rdx)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xe7d>
cmp    $0x2,%cl
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a4b>
mov    $0x2,%r9d
cmp    %al,0xa(%rdx)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xe7d>
cmp    $0x3,%ecx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a4b>
mov    %r11,0x38(%rsp)
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1db6>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d95>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d95>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1c13>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b2b1(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d95>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1c53>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b2f1(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d95>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1c93>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b331(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d95>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cd3>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b371(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d95>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d13>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b3b1(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d95>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d4f>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b3ed(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d95>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d8b>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b429(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %r13,%rdi
mov    %r14,%rsi
call   ADDR <expanse_trie::mutate::upgrade_l3_to_l7::<false>>
mov    0x38(%rsp),%r11
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
movzbl 0x9(%rdx),%r9d
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1de4>
mov    $0x2,%r8d
cmp    %al,%r9b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2176>
mov    $0x1,%r8d
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2176>
mov    %r9b,0xa(%rdx)
movdqu 0x20(%rdx),%xmm0
movdqu %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2176>
mov    0x40(%rsp),%rbx
mov    (%rbx),%rax
test   %rax,%rax
mov    0x20(%rsp),%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2016>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ff5>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ff5>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1e73>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b511(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ff5>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1eb3>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b551(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ff5>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ef3>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b591(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ff5>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f33>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b5d1(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ff5>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f73>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b611(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ff5>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1faf>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b64d(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ff5>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1feb>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29b689(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %r13,%rdi
mov    %r14,%rsi
call   ADDR <expanse_trie::mutate::upgrade_b_to_u::<false>>
mov    0x38(%rsp),%r11
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
cmp    $0x3,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2075>
cmp    %dl,0xa(%r14)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2075>
mov    0x38(%rsp),%r11
cmp    $0x7,%r11
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6285>
mov    %r11,%rax
shl    $0x4,%rax
mov    0x20(%rsp),%rcx
mov    %rcx,0xc8(%rsp,%rax,1)
mov    %bpl,0xd0(%rsp,%rax,1)
mov    $0x2,%ecx
mov    0x28(%rsp),%r12
mov    0x18(%rsp),%r13
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1b66>
lea    0x8(%r14),%rdi
lea    0x10(%r14),%rbx
movzbl %dl,%r8d
mov    $0x7,%edx
mov    %rbx,%rsi
mov    %rax,%rcx
call   *0xcb206(%rip)        # ADDR <_DYNAMIC+0x5e0>
incb   0x4(%r14)
or     %r12w,0x6(%r14)
mov    0x38(%rsp),%r11
cmp    $0x8,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62c6>
mov    %r11,%rcx
shl    $0x4,%rcx
mov    0x20(%rsp),%rdx
mov    %rdx,0xc8(%rsp,%rcx,1)
mov    %bpl,0xd0(%rsp,%rcx,1)
cmp    $0x7,%eax
mov    0x28(%rsp),%r12
mov    0x18(%rsp),%r13
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x62db>
inc    %r11
shl    $0x4,%eax
add    %rax,%rbx
mov    %r15d,%ebp
mov    %rbx,%r14
mov    0x40(%rsp),%rbx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    $0x1,%eax
mov    %eax,%r15d
shl    $0x4,%r15d
mov    0x18(%rsp),%rcx
add    %r15,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x17,%rax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x22ab>
lea    -0x2996c3(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%r15,%rcx,1),%r12d
cmp    $0xff,%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x22ab>
cmp    $0x3d,%r12b
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6319>
mov    0x18(%rsp),%rcx
mov    0x10(%rcx,%r12,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x21f3>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%r12,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x229b>
mov    %r8b,0x9(%rdx)
movdqu (%r9),%xmm0
movdqu %xmm0,0x20(%rdx)
xor    %r8d,%r8d
mov    %al,0x8(%rdx,%r8,1)
shl    $0x4,%r8d
pxor   %xmm0,%xmm0
movdqu %xmm0,0x10(%rdx,%r8,1)
inc    %cl
mov    %cl,0x4(%rdx)
and    $0xf,%al
mov    $0x1,%r9d
mov    %eax,%ecx
shl    %cl,%r9d
or     %r9w,0x6(%rdx)
cmp    $0x8,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6260>
mov    %r11,%rax
shl    $0x4,%rax
mov    %r14,0xc8(%rsp,%rax,1)
lea    (%rdx,%r8,1),%r14
add    $0x10,%r14
mov    %sil,0xd0(%rsp,%rax,1)
inc    %r11
mov    %edi,%ebp
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
cmp    $0x18,%r13w
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2452>
mov    $0x18,%eax
cmp    $0x10,%r13d
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x20f7>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2484>
cmp    $0x11,%eax
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x22ab>
movq   $0x0,0x58(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x58(%rsp),%rdi
call   *0xcad96(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61a6>
mov    0x58(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x61a6>
mov    $0x1000,%edx
xor    %esi,%esi
mov    %rdi,0x50(%rsp)
call   *0xcad7e(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x50(%rsp),%rdi
mov    0x18(%rsp),%r8
mov    0x220(%r8),%rax
mov    %rax,(%rdi)
mov    %r12,0x8(%rdi)
mov    %rdi,0x220(%r8)
mov    $0xfc0,%eax
xor    %edx,%edx
div    %r15d
nopl   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %r15,%rcx
lea    (%rdi,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x10(%r8,%r12,8),%rsi
mov    %rsi,0x40(%rdi,%rcx,1)
mov    %rdx,0x10(%r8,%r12,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2270>
add    $0x40,%rdi
xor    %esi,%esi
mov    %r15,%rdx
mov    %rdi,%r12
call   *0xcad17(%rip)        # ADDR <memset@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x22c5>
mov    $0x1,%esi
mov    %r15,%rdi
call   *0xcacff(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r12
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x63eb>
test   %r13,%r13
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x23c0>
mov    0x20(%rbp,%r14,8),%rsi
mov    %rsi,0x98(%rsp)
mov    0x30(%rsp),%r15
shl    $0x4,%r15d
mov    %r12,%rdi
mov    %r15,%rdx
call   *0xcabe0(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r12,0x50(%rsp)
lea    (%r12,%r15,1),%rdi
add    $0x10,%rdi
mov    0x98(%rsp),%r12
add    %r12,%r15
mov    %r13,%rdx
sub    0x30(%rsp),%rdx
shl    $0x4,%rdx
mov    %r15,%rsi
call   *0xcabb3(%rip)        # ADDR <memcpy@GLIBC_2.14>
test   %r12,%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60cf>
mov    %r12,%rsi
mov    %r13,%rax
cmp    $0x3,%r13w
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2362>
cmp    $0x11,%r13w
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2343>
lea    0x3(%r13),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2362>
mov    $0x18,%eax
cmp    $0x19,%r13w
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2362>
mov    $0x20,%eax
cmp    $0x21,%r13w
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2362>
lea    0x3(%r13),%eax
and    $0xfffffffc,%eax
mov    %eax,%ecx
shl    $0x4,%ecx
mov    0x18(%rsp),%rdx
sub    %rcx,0x200(%rdx)
decq   0x208(%rdx)
cmp    $0x17,%rax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x23b2>
lea    -0x299921(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rcx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x23b2>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6307>
mov    0x18(%rsp),%rcx
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rsi)
mov    %rsi,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x23bb>
mov    %rsi,%rdi
call   *0xcab2d(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x50(%rsp),%r12
mov    0x30(%rsp),%rax
shl    $0x4,%eax
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12,%rax,1)
mov    %r12,0x20(%rbp,%r14,8)
mov    0x28(%rsp),%r12
mov    $0x1,%eax
mov    %ebx,%ecx
shl    %cl,%rax
inc    %r13d
mov    %r13w,0x60(%rbp,%r14,2)
shr    $0x3,%ebx
and    $0x18,%ebx
or     %rax,0x0(%rbp,%rbx,1)
mov    0x38(%rsp),%r11
cmp    $0x7,%r11
mov    0x18(%rsp),%r13
mov    0x70(%rsp),%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f2>
mov    %r11,%rax
shl    $0x4,%rax
mov    0x20(%rsp),%rcx
mov    %rcx,0xc8(%rsp,%rax,1)
mov    0x10(%rsp),%rcx
mov    %cl,0xd0(%rsp,%rax,1)
inc    %r11
mov    0x30(%rsp),%rax
shl    $0x4,%eax
add    0x20(%rbp,%r14,8),%rax
mov    %edx,%ebp
mov    %rax,%r14
mov    0x40(%rsp),%rbx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
cmp    $0x20,%r13w
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x246a>
mov    $0x20,%eax
cmp    $0x18,%r13d
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x20f7>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2484>
mov    $0x24,%eax
cmp    $0x20,%r13d
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x24c0>
lea    0x4(%r13),%rax
lea    0x3(%r13),%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x24c0>
mov    0x20(%rbp,%r14,8),%r15
mov    0x30(%rsp),%rax
mov    %eax,%r12d
shl    $0x4,%r12d
lea    (%r15,%r12,1),%rsi
lea    (%r15,%r12,1),%rdi
add    $0x10,%rdi
mov    %r13,%rdx
sub    %rax,%rdx
shl    $0x4,%rdx
call   *0xcaaef(%rip)        # ADDR <memmove@GLIBC_2.2.5>
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r15,%r12,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x23d7>
and    $0xfffffffc,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x20f7>
mov    %rdx,0x70(%rsp)
movzbl 0x8(%rsp),%ecx
mov    0x20(%rsp),%rax
mov    (%rax),%rdi
movzbl (%rdi,%rbp,1),%eax
mov    %r8,%r13
mov    %rcx,%r15
cmp    %rax,%rcx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d5c>
cmp    $0x18,%ebp
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f10>
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x355a>
cmp    $0xe,%ebp
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x39eb>
lea    0x5(%r14),%edx
and    $0x1c,%edx
cmp    $0x1,%ebp
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3a07>
mov    0x20(%rsp),%rcx
movzwl 0xd(%rcx),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%rcx),%eax
mov    %eax,0x48(%rsp)
mov    $0x2,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3579>
mov    (%r14),%rax
mov    0x8(%rsp),%rsi
mov    %esi,%ecx
shr    $0x3,%ecx
and    $0x18,%ecx
mov    (%rax,%rcx,1),%rdx
bts    %rsi,%rdx
mov    %rdx,(%rax,%rcx,1)
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6c>
mov    0x8(%r14),%rcx
movzbl %cl,%eax
add    $0x2,%rax
cmp    $0x100,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3469>
cmp    $0x1,%bpl
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3469>
mov    (%r14),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x614e>
addq   $0xffffffffffffffc0,0x200(%r13)
decq   0x208(%r13)
mov    0x18(%r13),%rcx
mov    %rcx,(%rax)
mov    %rax,0x18(%r13)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r14)
movb   $0x7f,0xf(%r14)
movb   $0xff,0x8(%r14)
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x34bd>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2798>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2798>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2616>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29bcb4(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2798>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2656>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29bcf4(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2798>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2696>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29bd34(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2798>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x26d6>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29bd74(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2798>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2716>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29bdb4(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2798>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2752>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29bdf0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2798>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x278e>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29be2c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x34bd>
test   %bpl,%bpl
js     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60b6>
mov    %ebp,%r14d
shr    $0x4,%r14d
test   %r14b,%r14b
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60b6>
mov    %ebp,%r15d
and    $0xf,%r15d
lea    0x1(%r15),%r13
mov    %r13,%rbx
imul   %r14,%rbx
cmp    $0xf,%rbx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60b6>
mov    0x40(%rsp),%rax
mov    (%rax),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a34>
mov    0x28(%rsp),%rdx
mov    0x68(%rdx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a11>
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a11>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x285c>
mov    0x28(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29befa(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
mov    0x28(%rsp),%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a11>
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x28a3>
mov    0x28(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29bf41(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
mov    0x28(%rsp),%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a11>
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x28ea>
mov    0x28(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29bf88(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
mov    0x28(%rsp),%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a11>
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2931>
mov    0x28(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29bfcf(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
mov    0x28(%rsp),%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a11>
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2978>
mov    0x28(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c016(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
mov    0x28(%rsp),%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a11>
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x29bf>
mov    0x28(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c05d(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
mov    0x28(%rsp),%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a11>
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a02>
mov    0x28(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c0a0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
mov    0x28(%rsp),%rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%rdx)
movw   $0x0,0x70(%rdx)
pxor   %xmm0,%xmm0
mov    0x40(%rsp),%rax
movdqu %xmm0,(%rax)
movq   $0x0,0x10(%rax)
lea    0x0(,%r14,8),%ecx
mov    $0xffffffffffffffff,%rax
shl    %cl,%rax
not    %rax
mov    0x8(%rsp),%r8
and    %rax,%r8
test   %r15b,%r15b
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x33ed>
mov    0x20(%rsp),%r12
mov    (%r12),%rax
mov    0xb(%r12),%ecx
mov    %ecx,0x63(%rsp)
mov    0x8(%r12),%ecx
mov    %ecx,0x60(%rsp)
mov    %rax,0x58(%rsp)
movb   $0x0,0x67(%rsp)
movzbl %r14b,%edx
lea    0x58(%rsp),%rdi
mov    %r13,%rsi
mov    %rdx,0x30(%rsp)
mov    %r8,0x8(%rsp)
mov    %r8,%rcx
call   ADDR <expanse_trie::leaf::locate>
test   $0x1,%al
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    %rax,0x10(%rsp)
lea    0x8(%r12),%rdi
mov    $0xf,%al
movzbl %al,%eax
div    %r14b
cmp    %al,%r13b
mov    %r13,%rsi
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3d26>
movdqu 0x58(%rsp),%xmm0
movdqa %xmm0,0xb0(%rsp)
cmp    %rdx,%rsi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2b10>
mov    %rdx,%r12
imul   %r14,%rdx
mov    %rdi,%r13
lea    0x1(%r12),%r8
imul   %r14,%r8
lea    0xc7fef(%rip),%r9        # ADDR <anon.ADDR.2.llvm.ADDR+0x78f0>
lea    0xb0(%rsp),%rdi
mov    $0x10,%esi
mov    %rbx,%rcx
call   ADDR <<[u8]>::copy_within::<core::ops::range::Range<usize>>>
mov    %r12,%rdx
mov    %r13,%rdi
lea    0x1(%r15),%rsi
mov    0x30(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5a57>
lea    -0x29fe77(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x8bfd>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
mov    0x8(%rsp),%r13
jmp    *%rcx
mov    %r13b,0xb0(%rsp,%rdx,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5aaf>
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d48>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d27>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d27>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ba5>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c243(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d27>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2be5>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c283(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d27>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2c25>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c2c3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d27>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2c65>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c303(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d27>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ca5>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c343(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d27>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ce1>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c37f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d27>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d1d>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29c3bb(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
cmp    $0x7,%r9b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x32e0>
mov    0x8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x32fd>
mov    %r15,%r9
cmp    %eax,%r9d
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6c>
mov    %r14d,%eax
and    $0xfc,%eax
cmp    $0xc,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x388f>
movdqu (%rdi),%xmm0
mov    0x8(%rsp),%rax
add    $0xffffff80,%eax
movzbl %al,%eax
movd   %eax,%xmm1
punpcklbw %xmm1,%xmm1
pshuflw $0x0,%xmm1,%xmm1
pshufd $0x44,%xmm1,%xmm1
pxor   -0x2c0952(%rip),%xmm0        # ADDR <anon.ADDR.407.llvm.ADDR+0x470>
pcmpgtb %xmm0,%xmm1
pmovmskb %xmm1,%eax
mov    $0xffffffff,%edx
mov    %r8d,%ecx
shl    %cl,%edx
not    %edx
and    %eax,%edx
mov    %edx,%eax
shr    $1,%eax
and    $0x5555,%eax
sub    %eax,%edx
mov    %edx,%eax
and    $0x33333333,%eax
shr    $0x2,%edx
and    $0x33333333,%edx
add    %eax,%edx
mov    %edx,%eax
shr    $0x4,%eax
add    %edx,%eax
and    $0xf0f0f0f,%eax
imul   $0x1010101,%eax,%edx
shr    $0x18,%edx
cmp    %rbp,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x38d7>
mov    %rdx,%r13
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x24ed>
lea    0x0(,%rbx,8),%r14d
mov    $0xffffffffffffffff,%r8
mov    %r14d,%ecx
shl    %cl,%r8
not    %r8
and    0x8(%rsp),%r8
mov    0x20(%rsp),%rax
mov    (%rax),%rax
test   %r11,%r11
mov    %rax,0x10(%rsp)
mov    %r8,0x50(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e5f>
lea    -0x2(%rbx),%eax
lea    -0x2a0151(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8c35>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    0x10(%rsp),%rax
movzwl (%rax,%r15,2),%eax
mov    %r11,0x70(%rsp)
cmp    %rax,%r8
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6a>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d58>
mov    0x0(%r13),%eax
mov    %r13,0x98(%rsp)
mov    0x3(%r13),%ecx
mov    %ecx,0x4b(%rsp)
mov    %eax,0x48(%rsp)
mov    $0x1,%r13d
xor    %edi,%edi
movq   $0x0,0x70(%rsp)
imul   %rbx,%r13
lea    0xf(%r13),%rax
movabs $0x7ffffffffffffff0,%r12
and    %rax,%r12
mov    0x18(%rsp),%rax
add    %r12,0x200(%rax)
incq   0x208(%rax)
incq   0x218(%rax)
lea    -0x29a458(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl 0x0(%r13,%rax,1),%r15d
cmp    $0xff,%r15
mov    %r11,0xa0(%rsp)
mov    %rsi,0x78(%rsp)
mov    %rdi,0x8(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x33ba>
cmp    $0x3d,%r15b
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x632e>
mov    0x18(%rsp),%rcx
mov    0x10(%rcx,%r15,8),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x38ed>
mov    (%r14),%rax
mov    %rax,0x10(%rcx,%r15,8)
mov    %rcx,%r12
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x398e>
lea    0x4(%rbp),%eax
and    $0xfffffffc,%eax
cmp    $0x20,%rbp
mov    $0x20,%ebp
cmovae %rax,%rbp
mov    0x20(%rsp),%rsi
movzwl 0xd(%rsi),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%rsi),%eax
mov    %eax,0x48(%rsp)
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3172>
mov    %r12,%rdx
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3154>
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3154>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2fa1>
mov    0x8(%r12),%r10
mov    %rdi,%r11
mov    0x8(%r10),%rdi
lea    -0x29c63c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r11,%rdi
mov    %r9,0x8(%r10)
cmp    $0x2,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3154>
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2fe8>
mov    0x10(%r12),%r10
mov    %rdi,%r11
mov    0x8(%r10),%rdi
lea    -0x29c683(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r11,%rdi
mov    %r9,0x8(%r10)
cmp    $0x3,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3154>
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x302f>
mov    0x18(%r12),%r10
mov    %rdi,%r11
mov    0x8(%r10),%rdi
lea    -0x29c6ca(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r11,%rdi
mov    %r9,0x8(%r10)
cmp    $0x4,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3154>
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3076>
mov    0x20(%r12),%r10
mov    %rdi,%r11
mov    0x8(%r10),%rdi
lea    -0x29c711(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r11,%rdi
mov    %r9,0x8(%r10)
cmp    $0x5,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3154>
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x30bd>
mov    0x28(%r12),%r10
mov    %rdi,%r11
mov    0x8(%r10),%rdi
lea    -0x29c758(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r11,%rdi
mov    %r9,0x8(%r10)
cmp    $0x6,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3154>
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3104>
mov    0x30(%r12),%r10
mov    %rdi,%r11
mov    0x8(%r10),%rdi
lea    -0x29c79f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r11,%rdi
mov    %r9,0x8(%r10)
cmp    $0x7,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3154>
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3147>
mov    0x38(%r12),%r10
mov    %rdi,%r11
mov    0x8(%r10),%rdi
lea    -0x29c7e2(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r11,%rdi
mov    %r9,0x8(%r10)
cmp    $0x8,%rax
mov    %r12,%rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%rdx)
movw   $0x0,0x70(%rdx)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %rdi,0x10(%rsp)
lea    0x58(%rsp),%rdi
mov    $0x1,%edx
mov    %r8,%rcx
call   *0xca186(%rip)        # ADDR <_DYNAMIC+0x658>
mov    0x68(%rsp),%rbx
cmp    %r13,%rbx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6101>
cmp    0x58(%rsp),%rbx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x31aa>
lea    0x58(%rsp),%rdi
call   *0xca656(%rip)        # ADDR <_DYNAMIC+0xb48>
lea    0x0(,%r13,8),%r14
add    0x60(%rsp),%r14
mov    %rbx,%rdx
sub    %r13,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x31d0>
lea    0x8(%r14),%rdi
shl    $0x3,%rdx
mov    %r14,%rsi
call   *0xc9dd0(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    0x1(%rbx),%rax
mov    %r15,(%r14)
mov    %rax,0x68(%rsp)
cmpb   $0x2,0x30(%rsp)
mov    0x20(%rsp),%r14
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32ac>
mov    0x30(%rsp),%rcx
lea    -0x1(%rcx),%esi
movzbl %sil,%edi
dec    %rdi
xor    %ecx,%ecx
lea    0xbf1cc(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%sil
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6275>
mov    %rcx,%r8
shl    $0x8,%r8
movzbl 0x9(%r14,%rdi,1),%ecx
or     %r8,%rcx
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3210>
shl    $0x8,%rcx
mov    0x60(%rsp),%rsi
mov    %rsi,%rdx
cmp    $0x3,%rbx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3290>
movabs $0x1ffffffffffffffc,%rdi
and    %rax,%rdi
lea    (%rsi,%rdi,8),%rdx
movq   %rcx,%xmm0
pshufd $0x44,%xmm0,%xmm0
xor    %r8d,%r8d
movdqu (%rsi,%r8,8),%xmm1
movdqu 0x10(%rsi,%r8,8),%xmm2
por    %xmm0,%xmm1
por    %xmm0,%xmm2
movdqu %xmm1,(%rsi,%r8,8)
movdqu %xmm2,0x10(%rsi,%r8,8)
add    $0x4,%r8
cmp    %r8,%rdi
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3260>
cmp    %rdi,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32ac>
lea    (%rsi,%rax,8),%rax
data16 data16 cs nopw 0x0(%rax,%rax,1)
or     %rcx,(%rdx)
add    $0x8,%rdx
cmp    %rax,%rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x32a0>
mov    0x18(%rsp),%rcx
addq   $0x40,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
mov    0x18(%rcx),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3586>
mov    (%rbx),%rax
mov    %rax,0x18(%rcx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x362d>
lea    0x0(,%r9,8),%ecx
mov    $0xffffffffffffffff,%rax
shl    %cl,%rax
not    %rax
mov    0x8(%rsp),%rcx
and    %rax,%rcx
movl   $0x0,0x5b(%rsp)
movl   $0x0,0x58(%rsp)
shl    $0x4,%r9b
mov    %rcx,(%r14)
mov    0x58(%rsp),%eax
mov    0x5b(%rsp),%ecx
mov    %eax,0x8(%r14)
mov    %ecx,0xb(%r14)
mov    %r9b,0xf(%r14)
mov    $0x1,%al
test   %r11,%r11
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
shl    $0x4,%r11
lea    (%rsp,%r11,1),%rcx
add    $0xc8,%rcx
lea    0xc8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3367>
nopl   0x0(%rax)
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3399>
mov    0x8(%rdi),%r8
lea    -0x29ca37(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%r9
mov    %r9,%r10
and    %r8,%r10
inc    %r10
xor    %r8,%r10
and    %r9,%r10
xor    %r8,%r10
mov    %r10,0x8(%rdi)
mov    (%rbx),%r8
lea    -0x1(%r8),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x335a>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3350>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bca>
cmp    $0xf,%r13
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc4>
mov    $0x1,%esi
mov    %r13,%rdi
call   *0xc9be6(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r14
test   %rax,%rax
mov    0x28(%rsp),%r15
mov    0x18(%rsp),%r12
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x39a1>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6380>
mov    0x20(%rsp),%rcx
and    (%rcx),%rax
cmp    %r8,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6c>
mov    %rax,%rcx
cmova  %r8,%rcx
cmovb  %r8,%rax
movq   $0x0,0x8d(%rsp)
movq   $0x0,0x88(%rsp)
movzbl %r14b,%edx
dec    %edx
cmp    $0x6,%edx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x58c5>
lea    -0x2a076b(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0x8c19>
movslq (%rsi,%rdx,4),%rdx
add    %rsi,%rdx
mov    0x40(%rsp),%r11
mov    0x38(%rsp),%rsi
mov    0x20(%rsp),%rdi
mov    0x28(%rsp),%rbx
jmp    *%rdx
mov    %cl,0x88(%rsp)
mov    %al,0x89(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x595e>
lea    0x1(%rcx),%edx
and    $0xffffffffffffff00,%rcx
movzbl %dl,%edx
or     %rcx,%rdx
mov    %rdx,0x8(%r14)
cmp    $0x1,%bpl
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x34bd>
mov    0x8(%rsp),%rcx
shr    $0x8,%rcx
mov    %rcx,0x40(%r12)
mov    (%r14),%rcx
mov    %rcx,0x58(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x60(%r12)
mov    %ax,0x70(%r12)
mov    %r14,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
mov    $0x1,%al
test   %r11,%r11
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
shl    $0x4,%r11
lea    (%rsp,%r11,1),%rcx
add    $0xc8,%rcx
lea    0xc8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3507>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3539>
mov    0x8(%rdi),%r8
lea    -0x29cbd7(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%r9
mov    %r9,%r10
and    %r8,%r10
inc    %r10
xor    %r8,%r10
and    %r9,%r10
xor    %r8,%r10
mov    %r10,0x8(%rdi)
mov    (%rbx),%r8
lea    -0x1(%r8),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x34fa>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x34f0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bca>
mov    0x20(%rsp),%rcx
movzwl 0xd(%rcx),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%rcx),%eax
mov    %eax,0x48(%rsp)
mov    $0x2,%edx
mov    $0x1,%esi
mov    0x18(%rsp),%rcx
mov    %r13,%r15
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4106>
movq   $0x0,0x80(%rsp)
lea    0x80(%rsp),%rdi
mov    $0x40,%esi
mov    $0x1000,%edx
call   *0xc9a06(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638e>
mov    0x80(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x638e>
mov    $0x1000,%edx
mov    %rbx,%rdi
xor    %esi,%esi
call   *0xc99ed(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x18(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%rbx)
movq   $0x1,0x8(%rbx)
mov    %rbx,0x220(%rdi)
lea    0xfc0(%rbx),%rax
mov    $0x41,%ecx
nopl   (%rax)
mov    0x18(%rdi),%rdx
mov    %rdx,(%rax)
mov    %rax,0x18(%rdi)
lea    -0x40(%rax),%rdx
mov    0x18(%rdi),%rsi
mov    %rsi,-0x40(%rax)
mov    %rdx,0x18(%rdi)
add    $0xffffffffffffff80,%rax
add    $0xfffffffffffffffe,%rcx
cmp    $0x3,%rcx
jg     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3600>
add    $0x40,%rbx
pxor   %xmm0,%xmm0
movdqu %xmm0,0x30(%rbx)
movdqu %xmm0,0x20(%rbx)
movdqu %xmm0,0x10(%rbx)
movdqu %xmm0,(%rbx)
mov    0x68(%rsp),%rdx
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x36c4>
mov    0x60(%rsp),%rsi
lea    0x0(,%rdx,8),%rdi
add    $0xfffffffffffffff8,%rdi
mov    %rsi,%rax
test   $0x8,%dil
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3682>
mov    (%rsi),%rcx
mov    $0x1,%r8d
shl    %cl,%r8
lea    0x8(%rsi),%rax
shr    $0x3,%ecx
and    $0x18,%ecx
or     %r8,(%rbx,%rcx,1)
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x36c4>
lea    (%rsi,%rdx,8),%rdx
nopl   0x0(%rax,%rax,1)
mov    (%rax),%rcx
mov    $0x1,%esi
shl    %cl,%rsi
shr    $0x3,%ecx
and    $0x18,%ecx
or     %rsi,(%rbx,%rcx,1)
mov    0x8(%rax),%rcx
mov    $0x1,%esi
shl    %cl,%rsi
add    $0x10,%rax
shr    $0x3,%ecx
and    $0x18,%ecx
or     %rsi,(%rbx,%rcx,1)
cmp    %rdx,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3690>
movl   $0x0,0x83(%rsp)
movl   $0x0,0x80(%rsp)
mov    %rbx,(%r14)
mov    0x80(%rsp),%eax
mov    0x83(%rsp),%ecx
mov    0x70(%rsp),%r15
mov    %ecx,0x3(%r15)
mov    %eax,(%r15)
movb   $0xc,0xf(%r14)
movzbl 0x68(%rsp),%eax
dec    %al
mov    %al,0x8(%r14)
mov    0x30(%rsp),%rax
cmp    $0x2,%al
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3734>
add    $0x9,%r14
movzbl %al,%edx
dec    %rdx
lea    0x48(%rsp),%rsi
mov    %r14,%rdi
call   *0xc97a8(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x38(%rsp),%r14
mov    0x18(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x378a>
cmp    $0x1,%al
mov    0x38(%rsp),%r14
mov    0x18(%rsp),%rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x378a>
mov    0x8(%rsp),%rax
shr    $0x8,%rax
mov    %rax,0x40(%r12)
mov    %rbx,0x58(%r12)
movq   $0x0,0x60(%r12)
movzwl 0x68(%rsp),%eax
mov    %ax,0x70(%r12)
mov    0x20(%rsp),%rax
mov    %rax,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
lea    0xc(%rbp),%eax
and    $0xfffffff0,%eax
sub    %rax,0x200(%rdx)
decq   0x208(%rdx)
lea    -0x29ad3f(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl 0x0(%rbp,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x37de>
cmp    $0x3e,%dil
mov    0x40(%rsp),%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6343>
mov    0x10(%rdx,%rdi,8),%rax
mov    0x10(%rsp),%rcx
mov    %rax,(%rcx)
mov    %rcx,0x10(%rdx,%rdi,8)
test   %r14,%r14
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x37f7>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5831>
mov    0x10(%rsp),%rdi
call   *0xc96ff(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x40(%rsp),%r11
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5831>
shl    $0x4,%r14
lea    (%rsp,%r14,1),%rax
add    $0xc8,%rax
lea    0xc8(%rsp),%rcx
lea    0xc6d14(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3837>
cs nopw 0x0(%rax,%rax,1)
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5831>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3869>
mov    0x8(%rdi),%r8
lea    -0x29cf07(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%r9
mov    %r9,%r10
and    %r8,%r10
inc    %r10
xor    %r8,%r10
and    %r9,%r10
xor    %r8,%r10
mov    %r10,0x8(%rdi)
mov    (%r11),%r8
lea    -0x1(%r8),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x382a>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3820>
mov    $0x8,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x627a>
cmp    $0x4,%ebp
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3dbe>
xor    %edx,%edx
mov    0x8(%rsp),%rax
cmp    %al,(%rdi)
setb   %dl
cmp    $0x1,%r8d
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2dea>
cmp    $0x2,%r8d
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5546>
cmp    $0x3,%r8d
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5557>
xor    %eax,%eax
mov    0x8(%rsp),%rcx
cmp    %cl,0x1(%rdi)
setb   %al
cmp    %cl,0x2(%rdi)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x556e>
mov    0x8(%rsp),%rax
mov    %rdx,%r13
cmp    %al,(%rdi,%rdx,1)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x24ed>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6c>
movq   $0x0,0x58(%rsp)
lea    0x58(%rsp),%rdi
mov    $0x40,%esi
mov    $0x1000,%edx
call   *0xc96a5(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61a6>
mov    0x58(%rsp),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x61a6>
mov    $0x1000,%edx
mov    %r14,%rdi
xor    %esi,%esi
call   *0xc968f(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x18(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%r14)
mov    %r15,0x8(%r14)
mov    %r14,0x220(%rdi)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%eax
nopl   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %r12,%rcx
lea    (%r14,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x10(%rdi,%r15,8),%rsi
mov    %rsi,0x40(%r14,%rcx,1)
mov    %rdx,0x10(%rdi,%r15,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3960>
add    $0x40,%r14
mov    %rdi,%r12
mov    %r14,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0xc9624(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x28(%rsp),%r15
lea    -0x2(%rbx),%eax
cmp    $0x4,%eax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3c1a>
lea    -0x2a0c88(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8c79>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
mov    0x10(%rsp),%r13
mov    0x70(%rsp),%rcx
jmp    *%rax
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x47d8>
mov    %rcx,%rbp
lea    (%rcx,%rcx,1),%rbx
mov    %r14,%rdi
mov    %r13,%rsi
mov    %rbx,%rdx
call   *0xc94ea(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x47dd>
xor    %eax,%eax
cmp    $0x17,%rbp
sete   %al
lea    0x18(,%rax,8),%rdx
mov    $0x18,%eax
cmp    $0xf,%rbp
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3a0e>
lea    0x4(%r14),%eax
and    $0x1c,%eax
cmp    %eax,%edx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3ce4>
cmp    %rbp,%r13
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3a42>
lea    (%rdi,%r13,1),%rsi
mov    %rdi,%r14
add    %r13,%rdi
inc    %rdi
mov    %r8,%rdx
sub    %r13,%rdx
mov    %r8,%rbx
call   *0xc9569(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r14,%rdi
mov    %rbx,%r8
mov    0x40(%rsp),%rbx
mov    0x8(%rsp),%rcx
mov    %cl,(%rdi,%r13,1)
mov    $0xffffffffffffff00,%rax
mov    0x70(%rsp),%r15
and    (%r15),%rax
or     %r8,%rax
mov    %rax,(%r15)
cmpb   $0x1,0x30(%rsp)
mov    0x20(%rsp),%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3e0c>
shr    $0x8,%rcx
mov    %rcx,0x40(%r12)
movq   $0x0,0x58(%r12)
mov    %rdi,0x60(%r12)
add    $0x2,%ebp
mov    %bp,0x70(%r12)
mov    %rax,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
mov    $0x1,%esi
mov    0x38(%rsp),%r9
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x403a>
lea    (%r15,%r15,2),%rax
mov    0x10(%rsp),%rdx
mov    (%rdx,%rax,2),%ecx
movzwl 0x4(%rdx,%rax,2),%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3afb>
mov    0x10(%rsp),%rax
mov    (%rax,%r15,4),%eax
mov    %r11,0x70(%rsp)
cmp    %rax,%r8
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6a>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d58>
lea    (%r15,%r15,4),%rax
mov    0x10(%rsp),%rdx
mov    (%rdx,%rax,1),%ecx
movzbl 0x4(%rdx,%rax,1),%eax
shl    $0x20,%rax
or     %rcx,%rax
mov    %r11,0x70(%rsp)
cmp    %rax,%r8
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6a>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d58>
lea    (%r15,%r15,2),%rax
mov    0x10(%rsp),%rdx
movzwl (%rdx,%rax,1),%ecx
movzbl 0x2(%rdx,%rax,1),%eax
shl    $0x10,%eax
or     %rcx,%rax
mov    %r11,0x70(%rsp)
cmp    %rax,%r8
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6a>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d58>
mov    0x10(%rsp),%rax
lea    (%rax,%r15,8),%rax
sub    %r15,%rax
mov    (%rax),%ecx
movzwl 0x4(%rax),%edx
movzbl 0x6(%rax),%eax
shl    $0x20,%rdx
or     %rcx,%rdx
shl    $0x30,%rax
or     %rdx,%rax
mov    %r11,0x70(%rsp)
cmp    %rax,%r8
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d58>
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b73>
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
lea    -0x2(%rbx),%eax
cmp    $0x4,%eax
mov    %rsi,0x78(%rsp)
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4cf7>
lea    -0x2a0e8b(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8c4d>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    0x10(%rsp),%rdi
mov    %r11,%rsi
mov    %r8,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<2>>
lea    0x1(%r15),%r11
cmp    %r11,%rax
mov    %rax,0x70(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d4e>
mov    0x10(%rsp),%rcx
movzwl (%rcx,%rax,2),%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c79>
movq   $0x0,0x58(%rsp)
lea    0x58(%rsp),%rdi
mov    $0x10,%esi
mov    %r13,%rdx
call   *0xc93d0(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
mov    0x28(%rsp),%r15
mov    0x18(%rsp),%r12
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6380>
mov    0x58(%rsp),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6380>
mov    %r14,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0xc93b2(%rip)        # ADDR <memset@GLIBC_2.2.5>
lea    -0x2(%rbx),%eax
cmp    $0x4,%eax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x39ad>
mov    0x70(%rsp),%rbp
test   %rbp,%rbp
mov    0x10(%rsp),%r13
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4843>
lea    0x0(,%rbp,8),%rbx
sub    %rbp,%rbx
mov    %r14,%rdi
mov    %r13,%rsi
mov    %rbx,%rdx
call   *0xc9289(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4845>
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4724>
lea    (%rcx,%rcx,1),%rax
mov    %rcx,%rbp
lea    (%rax,%rax,2),%rbx
mov    %r14,%rdi
mov    %r13,%rsi
mov    %rbx,%rdx
call   *0xc9261(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4729>
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4763>
mov    %rcx,%rbp
lea    0x0(,%rcx,4),%rbx
mov    %r14,%rdi
mov    %r13,%rsi
mov    %rbx,%rdx
call   *0xc9239(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4768>
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4799>
mov    %rcx,%rbp
lea    (%rcx,%rcx,4),%rbx
mov    %r14,%rdi
mov    %r13,%rsi
mov    %rbx,%rdx
call   *0xc9215(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x479e>
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x480b>
mov    %rcx,%rbp
lea    (%rcx,%rcx,2),%rbx
mov    %r14,%rdi
mov    %r13,%rsi
mov    %rbx,%rdx
call   *0xc91f1(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4810>
cmp    $0x10,%ebp
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x40d4>
lea    0x4(%r14),%esi
and    $0x1c,%esi
mov    0x20(%rsp),%rcx
movzwl 0xd(%rcx),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%rcx),%eax
mov    %eax,0x48(%rsp)
cmp    $0xf,%ebp
mov    0x18(%rsp),%rcx
mov    %r13,%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x40f6>
lea    0x5(%r14),%edx
and    $0x1c,%edx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4106>
lea    0x4(%rsi),%eax
and    $0xfffffffc,%eax
cmp    $0xf,%r15b
mov    $0x18,%r8d
cmovne %rax,%r8
imul   %r14,%r8
lea    0xc(%r8),%r15
mov    %r15d,%r12d
and    $0x3f0,%r12d
mov    0x18(%rsp),%rax
add    %r12,0x200(%rax)
incq   0x208(%rax)
incq   0x218(%rax)
lea    -0x29b307(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%r8,%rax,1),%ebx
cmp    $0xff,%rbx
mov    %rsi,0xc0(%rsp)
mov    %rdi,0x70(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x54c3>
cmp    $0x3d,%bl
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6354>
mov    0x18(%rsp),%rcx
mov    0x10(%rcx,%rbx,8),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x535e>
mov    %rdx,%r13
mov    0x0(%rbp),%rax
mov    %rax,0x10(%rcx,%rbx,8)
mov    0x28(%rsp),%rbx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x54ae>
cmp    $0x8,%ebp
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bdc>
mov    0x8(%rsp),%rax
add    $0xffffff80,%eax
movzbl %al,%eax
movd   %eax,%xmm0
punpcklbw %xmm0,%xmm0
pshuflw $0x0,%xmm0,%xmm0
movabs $0x8080808080808080,%rax
xor    (%rdi),%rax
pshufd $0x44,%xmm0,%xmm0
movq   %rax,%xmm1
shufpd $0x2,-0x2c1b0f(%rip),%xmm1        # ADDR <anon.ADDR.407.llvm.ADDR+0x310>
pcmpgtb %xmm1,%xmm0
pmovmskb %xmm0,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2daa>
mov    (%rbx),%rax
test   %rax,%rax
mov    0x38(%rsp),%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4038>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4017>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4017>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3e74>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29d512(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4017>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3eb4>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29d552(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4017>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3ef4>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29d592(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4017>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f34>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29d5d2(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4017>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f74>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29d612(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4017>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3fb4>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29d652(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
mov    0x40(%rsp),%rbx
mov    0x38(%rsp),%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4017>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3ffe>
mov    0x28(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29d69c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
mov    0x28(%rsp),%r12
mov    0x40(%rsp),%rbx
mov    0x38(%rsp),%r9
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
xor    %esi,%esi
mov    $0x1,%al
test   %r9,%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
shl    $0x4,%r9
lea    (%rsp,%r9,1),%rcx
add    $0xc8,%rcx
lea    0xc8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x406d>
xchg   %ax,%ax
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%r8d
cmp    $0x7,%r8
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x40a3>
mov    0x8(%rdi),%rsi
lea    -0x29d73e(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%r8,8),%r9
mov    %r9,%r10
and    %rsi,%r10
inc    %r10
xor    %rsi,%r10
and    %r9,%r10
xor    %rsi,%r10
mov    %r10,0x8(%rdi)
mov    (%rbx),%rsi
lea    -0x1(%rsi),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4060>
mov    %rdi,(%r12,%rsi,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bca>
mov    %r8b,0x48(%r12,%rdi,1)
mov    0x50(%r12),%rsi
inc    %rsi
mov    %rsi,0x50(%r12)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4060>
mov    0x20(%rsp),%rcx
movzwl 0xd(%rcx),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%rcx),%eax
mov    %eax,0x48(%rsp)
mov    $0x18,%esi
mov    0x18(%rsp),%rcx
mov    %r13,%r15
xor    %eax,%eax
cmp    $0x17,%ebp
sete   %al
lea    0x18(,%rax,8),%rdx
lea    0xe(%rdx),%r8
mov    %r8d,%r9d
and    $0xfffffff0,%r9d
add    %r9,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
lea    -0x29b6c7(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rax,1),%ebx
cmp    $0xff,%rbx
mov    %rdi,0x10(%rsp)
mov    %rsi,0xc0(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x416d>
cmp    $0x3d,%bl
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6354>
mov    0x10(%rcx,%rbx,8),%r12
test   %r12,%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x419d>
mov    (%r12),%rax
mov    %rax,0x10(%rcx,%rbx,8)
mov    0x28(%rsp),%rbx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4306>
cmp    $0xf,%edx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x430d>
mov    $0x1,%esi
mov    %rdx,%r13
mov    %rdx,%rdi
call   *0xc8e31(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r12
test   %rax,%rax
mov    0x28(%rsp),%rbx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4352>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6380>
mov    %r9,%r15
mov    %r8,0x50(%rsp)
mov    %rdx,0x70(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x61a6>
mov    %rax,%r12
mov    0x18(%rsp),%rcx
mov    0x220(%rcx),%rax
mov    %rax,(%r12)
mov    %rbx,0x8(%r12)
mov    %r12,0x220(%rcx)
test   %r15,%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x63c2>
mov    %r15,%r9
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r9w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
mov    0x18(%rsp),%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4236>
mov    %rcx,%rax
xchg   %ax,%ax
dec    %rax
mov    %rax,%rsi
imul   %r9,%rsi
lea    (%r12,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%r15,%rbx,8),%r8
mov    %r8,0x40(%r12,%rsi,1)
mov    %rdi,0x10(%r15,%rbx,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4210>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x42f5>
mov    0x50(%rsp),%r9
shr    $0x4,%r9d
lea    -0x4(%rax),%rcx
imul   %r9,%rcx
shl    $0x4,%rcx
add    $0x40,%rcx
mov    %r9d,%edx
shl    $0x6,%edx
neg    %rdx
lea    -0x3(%rax),%rsi
imul   %r9,%rsi
shl    $0x4,%rsi
add    $0x40,%rsi
lea    -0x2(%rax),%rdi
imul   %r9,%rdi
shl    $0x4,%rdi
add    $0x40,%rdi
lea    -0x1(%rax),%r8
imul   %r9,%r8
shl    $0x4,%r8
add    $0x40,%r8
mov    %r12,%r9
nopl   0x0(%rax,%rax,1)
lea    (%r9,%r8,1),%r10
mov    0x10(%r15,%rbx,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x10(%r15,%rbx,8)
lea    (%r9,%rdi,1),%r10
mov    0x10(%r15,%rbx,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%r15,%rbx,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%r15,%rbx,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%r15,%rbx,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%r15,%rbx,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%r15,%rbx,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x42a0>
add    $0x40,%r12
mov    0x28(%rsp),%rbx
mov    %r13,%r15
mov    0x70(%rsp),%rdx
mov    %r12,%rdi
xor    %esi,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x434c>
movq   $0x0,0x58(%rsp)
lea    0x58(%rsp),%rdi
mov    $0x10,%esi
mov    %rdx,%r13
call   *0xc8c87(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
mov    0x28(%rsp),%rbx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6380>
mov    0x58(%rsp),%r12
test   %r12,%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6380>
mov    %r12,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0xc8c6e(%rip)        # ADDR <memset@GLIBC_2.2.5>
test   %r15,%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4368>
mov    %r12,%rdi
mov    0x10(%rsp),%rsi
mov    %r15,%rdx
call   *0xc8b68(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x8(%rsp),%rax
mov    %al,(%r12,%r15,1)
cmp    %rbp,%r15
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4393>
lea    (%r12,%r15,1),%rdi
mov    0x10(%rsp),%rax
lea    (%rax,%r15,1),%rsi
inc    %rdi
lea    0x1(%rbp),%rdx
sub    %r15,%rdx
call   *0xc8b3d(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r15
mov    %r12,(%r15)
mov    %r14b,0x8(%r15)
mov    %r15,%r14
mov    0x48(%rsp),%eax
mov    %eax,0x9(%r15)
movzwl 0x4c(%rsp),%eax
mov    %ax,0xd(%r15)
mov    $0xffffffffffffff00,%rcx
and    0x8(%r15),%rcx
lea    0x1(%rbp),%rax
or     %rax,%rcx
mov    %rcx,0x8(%r15)
mov    0xc0(%rsp),%rdx
lea    0xf(%rdx),%eax
and    $0xfffffff0,%eax
mov    0x18(%rsp),%rcx
sub    %rax,0x200(%rcx)
decq   0x208(%rcx)
lea    -0x29b98c(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4468>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6307>
mov    0x18(%rsp),%rcx
mov    0x10(%rcx,%rdi,8),%rax
mov    0x10(%rsp),%rdx
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
cmpb   $0x1,0x30(%rsp)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x447a>
mov    0x8(%rsp),%rax
shr    $0x8,%rax
mov    %rax,0x40(%rbx)
movq   $0x0,0x58(%rbx)
mov    %r12,0x60(%rbx)
add    $0x2,%ebp
mov    %bp,0x70(%rbx)
mov    %r14,(%rbx)
movb   $0x1,0x48(%rbx)
movq   $0x1,0x50(%rbx)
movq   $0x0,0x68(%rbx)
mov    $0x1,%edx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x467b>
mov    0x10(%rsp),%rdi
call   *0xc8a75(%rip)        # ADDR <free@GLIBC_2.2.5>
cmpb   $0x1,0x30(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4427>
mov    0x40(%rsp),%rax
mov    (%rax),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4679>
mov    0x68(%rbx),%rcx
test   %rcx,%rcx
mov    0x38(%rsp),%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4652>
movq   $0x0,0x68(%rbx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4652>
movzbl 0x49(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x44e2>
mov    0x8(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29db80(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4652>
movzbl 0x4a(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x451f>
mov    0x10(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29dbbd(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4652>
movzbl 0x4b(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x455c>
mov    0x18(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29dbfa(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4652>
movzbl 0x4c(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4599>
mov    0x20(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29dc37(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4652>
movzbl 0x4d(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x45d6>
mov    0x28(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29dc74(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4652>
movzbl 0x4e(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x460f>
mov    0x30(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29dcad(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4652>
movzbl 0x4f(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4648>
mov    0x38(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29dce6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%rbx)
movw   $0x0,0x70(%rbx)
pxor   %xmm0,%xmm0
mov    0x40(%rsp),%rax
movdqu %xmm0,(%rax)
movq   $0x0,0x10(%rax)
xor    %edx,%edx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4680>
xor    %edx,%edx
mov    0x38(%rsp),%r9
test   %r9,%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5844>
shl    $0x4,%r9
lea    (%rsp,%r9,1),%rax
add    $0xc8,%rax
lea    0xc8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x46cd>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %sil,0x48(%rbx,%rdi,1)
mov    0x50(%rbx),%rdx
inc    %rdx
mov    %rdx,0x50(%rbx)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5844>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4707>
mov    0x8(%rdi),%rdx
lea    -0x29dd9d(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rsi,8),%r8
mov    %r8,%r9
and    %rdx,%r9
inc    %r9
xor    %rdx,%r9
and    %r8,%r9
xor    %rdx,%r9
mov    %r9,0x8(%rdi)
mov    0x40(%rsp),%rdx
mov    (%rdx),%rdx
lea    -0x1(%rdx),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46c0>
mov    %rdi,(%rbx,%rdx,8)
mov    0x50(%rbx),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x46b0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bca>
mov    %rcx,%rbp
xor    %ebx,%ebx
mov    0x50(%rsp),%rax
mov    %eax,(%r14,%rbx,1)
shr    $0x20,%rax
mov    %ax,0x4(%r14,%rbx,1)
mov    0xa0(%rsp),%r8
mov    %r8,%rax
sub    %rbp,%rax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4899>
lea    (%r14,%rbx,1),%rdi
add    %r13,%rbx
add    $0x6,%rdi
add    %rax,%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x483d>
mov    %rcx,%rbp
xor    %ebx,%ebx
mov    0x50(%rsp),%rax
mov    %eax,(%r14,%rbx,1)
mov    0xa0(%rsp),%r8
mov    %r8,%rdx
sub    %rbp,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4899>
lea    (%r14,%rbx,1),%rdi
add    %r13,%rbx
add    $0x4,%rdi
shl    $0x2,%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x488a>
mov    %rcx,%rbp
xor    %ebx,%ebx
mov    0x50(%rsp),%rax
mov    %eax,(%r14,%rbx,1)
shr    $0x20,%rax
mov    %al,0x4(%r14,%rbx,1)
mov    0xa0(%rsp),%r8
mov    %r8,%rax
sub    %rbp,%rax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4899>
lea    (%r14,%rbx,1),%rdi
add    %r13,%rbx
add    $0x5,%rdi
lea    (%rax,%rax,4),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x488a>
mov    %rcx,%rbp
xor    %ebx,%ebx
mov    0x50(%rsp),%rax
mov    %ax,(%r14,%rbx,1)
mov    0xa0(%rsp),%r8
mov    %r8,%rdx
sub    %rbp,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4899>
lea    (%r14,%rbx,1),%rdi
add    %r13,%rbx
add    $0x2,%rdi
add    %rdx,%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x488a>
mov    %rcx,%rbp
xor    %ebx,%ebx
mov    0x50(%rsp),%rax
mov    %ax,(%r14,%rbx,1)
shr    $0x10,%eax
mov    %al,0x2(%r14,%rbx,1)
mov    0xa0(%rsp),%r8
mov    %r8,%rax
sub    %rbp,%rax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4899>
lea    (%r14,%rbx,1),%rdi
add    %r13,%rbx
add    $0x3,%rdi
lea    (%rax,%rax,2),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x488a>
xor    %ebx,%ebx
mov    0x50(%rsp),%rcx
mov    %ecx,(%r14,%rbx,1)
mov    %rcx,%rax
shr    $0x20,%rax
mov    %ax,0x4(%r14,%rbx,1)
shr    $0x30,%rcx
mov    %cl,0x6(%r14,%rbx,1)
mov    0xa0(%rsp),%r8
mov    %r8,%rax
sub    %rbp,%rax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4899>
lea    (%r14,%rbx,1),%rdi
add    %r13,%rbx
add    $0x7,%rdi
lea    0x0(,%rax,8),%rdx
sub    %rax,%rdx
mov    %rbx,%rsi
mov    %r8,%rbx
call   *0xc863a(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbx,%r8
mov    0x20(%rsp),%rdx
mov    %r14,(%rdx)
mov    0x48(%rsp),%eax
mov    0x4b(%rsp),%ecx
mov    0x98(%rsp),%rsi
mov    %ecx,0x3(%rsi)
mov    %eax,(%rsi)
mov    0x8(%rdx),%rax
xor    %rax,%r8
and    0x78(%rsp),%r8
xor    %rax,%r8
mov    %r8,0x8(%rdx)
test   %r13,%r13
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60e8>
mov    0x8(%rsp),%rcx
lea    0xf(%rcx),%rax
and    $0xfffffffffffffff0,%rax
sub    %rax,0x200(%r12)
decq   0x208(%r12)
lea    -0x29be90(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rcx,%rax,1),%edi
cmp    $0xff,%rdi
mov    0x40(%rsp),%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x492d>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6307>
mov    0x10(%r12,%rdi,8),%rax
mov    %rax,0x0(%r13)
mov    %r13,0x10(%r12,%rdi,8)
mov    (%rbx),%rax
test   %rax,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4942>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b2a>
mov    %r13,%rdi
call   *0xc85b2(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b2a>
mov    0x68(%r15),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b0b>
movq   $0x0,0x68(%r15)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b0b>
movzbl 0x49(%r15),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4995>
mov    0x8(%r15),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29e033(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b0b>
movzbl 0x4a(%r15),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x49d3>
mov    0x10(%r15),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29e071(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b0b>
movzbl 0x4b(%r15),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4a11>
mov    0x18(%r15),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29e0af(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b0b>
movzbl 0x4c(%r15),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4a4f>
mov    0x20(%r15),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29e0ed(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b0b>
movzbl 0x4d(%r15),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4a8d>
mov    0x28(%r15),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29e12b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b0b>
movzbl 0x4e(%r15),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ac7>
mov    0x30(%r15),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29e165(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b0b>
movzbl 0x4f(%r15),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b01>
mov    0x38(%r15),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29e19f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r15)
movw   $0x0,0x70(%r15)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    0x38(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5844>
shl    $0x4,%rax
add    %rsp,%rax
add    $0xc8,%rax
lea    0xc8(%rsp),%rcx
xor    %edx,%edx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b7d>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %sil,0x48(%r15,%rdi,1)
mov    0x50(%r15),%rdx
inc    %rdx
mov    %rdx,0x50(%r15)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5844>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bb2>
mov    0x8(%rdi),%rdx
lea    -0x29e24d(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rsi,8),%r8
mov    %r8,%r9
and    %rdx,%r9
inc    %r9
xor    %rdx,%r9
and    %r8,%r9
xor    %rdx,%r9
mov    %r9,0x8(%rdi)
mov    (%rbx),%rdx
lea    -0x1(%rdx),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b70>
mov    %rdi,(%r15,%rdx,8)
mov    0x50(%r15),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b60>
lea    0xc5957(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
mov    $0x8,%esi
call   *0xc82e4(%rip)        # ADDR <_DYNAMIC+0x208>
xor    %edx,%edx
mov    %r8,%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bef>
mov    %rsi,%rdx
cmp    %rax,%rdx
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2dea>
mov    %rdx,%rsi
lea    (%rax,%rdx,1),%rcx
shr    $1,%rcx
movzbl (%rdi,%rcx,1),%edx
cmp    %rdx,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c05>
mov    %rcx,%rax
cmp    %edx,%r9d
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4be3>
inc    %rcx
mov    %rcx,%rdx
cmp    %rax,%rdx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bef>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2dea>
mov    0x10(%rsp),%rdi
mov    %r11,%rsi
mov    %r8,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<6>>
lea    0x1(%r15),%r11
cmp    %r11,%rax
mov    %rax,0x70(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d4e>
lea    (%rax,%rax,2),%rax
mov    0x10(%rsp),%rdx
mov    (%rdx,%rax,2),%ecx
movzwl 0x4(%rdx,%rax,2),%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4cb9>
mov    0x10(%rsp),%rdi
mov    %r11,%rsi
mov    %r8,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<4>>
lea    0x1(%r15),%r11
cmp    %r11,%rax
mov    %rax,0x70(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d4e>
mov    0x10(%rsp),%rcx
mov    (%rcx,%rax,4),%eax
mov    0x50(%rsp),%r8
cmp    %rax,%r8
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d41>
mov    0x10(%rsp),%rdi
mov    %r11,%rsi
mov    %r8,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<5>>
lea    0x1(%r15),%r11
cmp    %r11,%rax
mov    %rax,0x70(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d4e>
lea    (%rax,%rax,4),%rax
mov    0x10(%rsp),%rdx
mov    (%rdx,%rax,1),%ecx
movzbl 0x4(%rdx,%rax,1),%eax
shl    $0x20,%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4cf2>
mov    0x10(%rsp),%rdi
mov    %r11,%rsi
mov    %r8,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<3>>
lea    0x1(%r15),%r11
cmp    %r11,%rax
mov    %rax,0x70(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d4e>
lea    (%rax,%rax,2),%rax
mov    0x10(%rsp),%rdx
movzwl (%rdx,%rax,1),%ecx
movzbl 0x2(%rdx,%rax,1),%eax
shl    $0x10,%eax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d39>
mov    0x10(%rsp),%rdi
mov    %r11,%rsi
mov    %r8,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<7>>
lea    0x1(%r15),%r11
cmp    %r11,%rax
mov    %rax,0x70(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d4e>
mov    0x10(%rsp),%rcx
lea    (%rcx,%rax,8),%rsi
sub    %rax,%rsi
mov    (%rsi),%ecx
movzwl 0x4(%rsi),%edx
movzbl 0x6(%rsi),%eax
shl    $0x20,%rdx
or     %rcx,%rdx
shl    $0x30,%rax
or     %rdx,%rax
mov    0x50(%rsp),%r8
cmp    %r8,%rax
mov    0x78(%rsp),%rsi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b6c>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d58>
mov    0x78(%rsp),%rsi
mov    0x50(%rsp),%r8
cmp    $0x20,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4da3>
test   %r15,%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x501d>
cmp    $0xf,%r15
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5043>
lea    0x5(%r15),%rax
cmp    $0x3,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5094>
mov    %r11,%rdi
imul   %rbx,%rdi
mov    0x0(%r13),%ecx
mov    %r13,0x98(%rsp)
mov    0x3(%r13),%edx
mov    %edx,0x4b(%rsp)
mov    %ecx,0x48(%rsp)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5161>
and    $0xfffffffffffffffc,%r15
add    $0x4,%r15
cmp    $0x20,%r11
cmove  %r11,%r15
mov    0x0(%r13),%eax
mov    %r13,0x98(%rsp)
mov    0x3(%r13),%ecx
mov    %ecx,0x4b(%rsp)
mov    %eax,0x48(%rsp)
lea    0x58(%rsp),%rdi
mov    0x20(%rsp),%rsi
mov    %ebx,%edx
mov    %r11,%rcx
call   *0xc8530(%rip)        # ADDR <_DYNAMIC+0x658>
mov    0x68(%rsp),%rsi
cmp    0x70(%rsp),%rsi
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6119>
mov    %rsi,0x78(%rsp)
cmp    0x58(%rsp),%rsi
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4e07>
lea    0x58(%rsp),%rdi
call   *0xc89f9(%rip)        # ADDR <_DYNAMIC+0xb48>
mov    0x70(%rsp),%rax
lea    0x0(,%rax,8),%r13
add    0x60(%rsp),%r13
mov    0x78(%rsp),%rdx
sub    %rax,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4e34>
lea    0x8(%r13),%rdi
shl    $0x3,%rdx
mov    %r13,%rsi
call   *0xc816c(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x78(%rsp),%rax
lea    0x1(%rax),%r8
mov    0x50(%rsp),%rax
mov    %rax,0x0(%r13)
mov    %r8,0x68(%rsp)
mov    0x30(%rsp),%r10
cmp    %bl,%r10b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4fc1>
mov    %rbx,%rsi
xor    $0x7,%rsi
mov    %r10d,%eax
sub    %bl,%al
movzbl %al,%edi
dec    %rdi
lea    -0x4(%rbp),%eax
movzbl %al,%eax
mov    0x20(%rsp),%rcx
add    %rax,%rcx
add    $0x7,%rcx
xor    %eax,%eax
lea    0xbd54a(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
cs nopw 0x0(%rax,%rax,1)
cmp    %rsi,%rdi
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5826>
mov    %rax,%r9
shl    $0x8,%r9
movzbl 0x1(%rcx,%rdi,1),%eax
or     %r9,%rax
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4e90>
mov    %r14d,%ecx
shl    %cl,%rax
mov    0x60(%rsp),%rdx
mov    %rdx,%rcx
cmpq   $0x3,0x78(%rsp)
mov    0x20(%rsp),%rsi
mov    0x30(%rsp),%r10
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4f1c>
movabs $0x1ffffffffffffffc,%r9
and    %r8,%r9
lea    (%rdx,%r9,8),%rcx
movq   %rax,%xmm0
pshufd $0x44,%xmm0,%xmm0
xor    %edi,%edi
nopl   0x0(%rax,%rax,1)
movdqu (%rdx,%rdi,8),%xmm1
movdqu 0x10(%rdx,%rdi,8),%xmm2
por    %xmm0,%xmm1
por    %xmm0,%xmm2
movdqu %xmm1,(%rdx,%rdi,8)
movdqu %xmm2,0x10(%rdx,%rdi,8)
add    $0x4,%rdi
cmp    %rdi,%r9
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ef0>
cmp    %r9,%r8
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4f2c>
lea    (%rdx,%r8,8),%rdx
or     %rax,(%rcx)
add    $0x8,%rcx
cmp    %rdx,%rcx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4f20>
mov    0x68(%rsp),%r8
mov    0x60(%rsp),%rax
cmp    $0x21,%r8
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4fd5>
mov    -0x8(%rax,%r8,8),%rdx
xor    (%rax),%rdx
cmp    $0x7,%r10b
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4f66>
lea    0x0(,%r10,8),%ecx
mov    $0xffffffffffffffff,%rsi
shl    %cl,%rsi
not    %rsi
and    %rsi,%rdx
mov    $0x7f,%ecx
bsr    %rdx,%rcx
xor    $0x3f,%ecx
mov    0x18(%rsp),%rdx
addq   $0x40,0x200(%rdx)
mov    $0x3f,%r13b
incq   0x208(%rdx)
sub    %cl,%r13b
incq   0x218(%rdx)
mov    %r13d,%ebp
shr    $0x3,%bpl
mov    0x18(%rdx),%r14
test   %bpl,%bpl
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5056>
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x51bc>
mov    (%r14),%rax
mov    0x18(%rsp),%rcx
mov    %rax,0x18(%rcx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x522e>
mov    0x20(%rsp),%rsi
mov    0x60(%rsp),%rax
cmp    $0x21,%r8
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4f40>
mov    0x18(%rsp),%rdi
mov    %ebx,%edx
mov    %rax,%rcx
call   ADDR <expanse_trie::mutate::build_leaf::<false>>
mov    0x30(%rsp),%rax
cmp    %bl,%al
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x56f5>
movzbl %al,%edx
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61cf>
sub    %rbx,%rdx
lea    (%rsp,%rbx,1),%rsi
add    $0x48,%rsi
mov    0x98(%rsp),%rdi
add    %rbx,%rdi
call   *0xc7eb8(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x56f5>
mov    0x0(%r13),%eax
mov    %r13,0x98(%rsp)
mov    0x3(%r13),%ecx
mov    %ecx,0x4b(%rsp)
mov    %eax,0x48(%rsp)
mov    $0x2,%r13d
mov    %rbx,%rdi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e88>
xor    %eax,%eax
cmp    $0x17,%r15
setae  %al
lea    0x18(,%rax,8),%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5097>
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5576>
mov    (%r14),%rcx
mov    0x18(%rsp),%rdx
mov    %rcx,0x18(%rdx)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x30(%r14)
movdqu %xmm0,0x20(%r14)
movdqu %xmm0,0x10(%r14)
movdqu %xmm0,(%r14)
lea    (%rax,%r8,8),%rdx
mov    0x30(%rsp),%r8
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5620>
and    $0x1c,%eax
xor    %ecx,%ecx
cmp    $0x19,%r11
setae  %cl
mov    %r15,%rdi
and    $0xfffffffffffffffc,%rdi
add    $0x4,%rdi
cmp    $0x11,%r11
lea    0x18(,%rcx,8),%rcx
mov    %rcx,%rdx
cmovb  %rdi,%rdx
cmp    %rdx,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5128>
mov    %r11,%rdx
mov    0x70(%rsp),%r15
sub    %r15,%rdx
mov    %r11,0xa0(%rsp)
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5177>
mov    %rsi,0x78(%rsp)
mov    %r15,%rsi
imul   %rbx,%rsi
mov    0x10(%rsp),%rbp
add    %rbp,%rsi
lea    0x1(%r15),%rdi
imul   %rbx,%rdi
add    %rbp,%rdi
imul   %rbx,%rdx
mov    %r8,%r14
call   *0xc7e98(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r14,%r8
mov    0x78(%rsp),%rsi
lea    -0x2(%rbx),%eax
lea    -0x2a2406(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8c61>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
mov    0x38(%rsp),%r9
jmp    *%rax
mov    0x0(%r13),%eax
mov    %r13,0x98(%rsp)
mov    0x3(%r13),%edx
mov    %edx,0x4b(%rsp)
mov    %eax,0x48(%rsp)
cmp    $0x11,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x519f>
imul   %rbx,%rdi
mov    $0x18,%r13d
cmp    $0xe,%r15
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e88>
add    $0x5,%r15
mov    %r15,%rax
movabs $0x7ffffffffffffff0,%r13
or     $0xc,%r13
and    %rax,%r13
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e88>
lea    -0x2(%rbx),%eax
lea    -0x2a246d(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8c61>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
mov    0x38(%rsp),%r9
mov    0x10(%rsp),%rbp
jmp    *%rax
mov    %r8w,0x0(%rbp,%r15,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5dc7>
imul   %rbx,%rcx
xor    %eax,%eax
cmp    $0x17,%r15
setae  %al
lea    0x18(,%rax,8),%r13
mov    %rcx,%rdi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e88>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6201>
mov    %rax,%r14
mov    0x18(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%r14)
movq   $0x1,0x8(%r14)
mov    %r14,0x220(%rdi)
lea    0xfc0(%r14),%rax
mov    $0x41,%ecx
mov    0x18(%rdi),%rdx
mov    %rdx,(%rax)
mov    %rax,0x18(%rdi)
lea    -0x40(%rax),%rdx
mov    0x18(%rdi),%rsi
mov    %rsi,-0x40(%rax)
mov    %rdx,0x18(%rdi)
add    $0xffffffffffffff80,%rax
add    $0xfffffffffffffffe,%rcx
cmp    $0x3,%rcx
jg     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5201>
add    $0x40,%r14
inc    %bpl
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r14)
movdqu %xmm0,0x30(%r14)
movdqu %xmm0,0x20(%r14)
movdqu %xmm0,0x10(%r14)
mov    %bpl,0x5(%r14)
movl   $0x0,0x83(%rsp)
movl   $0x0,0x80(%rsp)
mov    0x20(%rsp),%r12
mov    %r14,(%r12)
mov    0x80(%rsp),%eax
mov    0x83(%rsp),%ecx
mov    0x98(%rsp),%rdx
mov    %ecx,0x3(%rdx)
mov    %eax,(%rdx)
movb   $0x1,0xf(%r12)
movzbl %bpl,%r14d
cmp    0x30(%rsp),%bpl
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x52c2>
cmpq   $0x0,0x68(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x63d8>
mov    0x60(%rsp),%rax
mov    (%rax),%rcx
movzbl 0x30(%rsp),%edx
mov    0x20(%rsp),%rdi
mov    %r14d,%esi
call   *0xc7fce(%rip)        # ADDR <_DYNAMIC+0x5d8>
mov    0x68(%rsp),%r12
test   %r12,%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5326>
mov    %r14,0x70(%rsp)
mov    %r13b,0x8(%rsp)
mov    0x60(%rsp),%r13
shl    $0x3,%r12
xor    %r14d,%r14d
movzbl 0x30(%rsp),%ebp
nopw   0x0(%rax,%rax,1)
mov    0x0(%r13,%r14,1),%rdx
mov    0x18(%rsp),%rdi
mov    0x20(%rsp),%rsi
mov    %ebp,%ecx
mov    0x28(%rsp),%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
add    $0x8,%r14
cmp    %r14,%r12
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x52f0>
mov    0x68(%rsp),%rax
movzbl 0x8(%rsp),%r13d
mov    0x70(%rsp),%r14
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5328>
xor    %eax,%eax
cmp    $0x38,%r13b
mov    0x28(%rsp),%r12
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63a0>
dec    %rax
mov    0x98(%rsp),%rsi
mov    (%rsi),%rcx
xor    %rcx,%rax
lea    -0x29ea07(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rdx,%r14,8),%rax
xor    %rcx,%rax
mov    %rax,(%rsi)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x56f5>
cmp    $0x101,%r8d
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x54c3>
mov    %r8,0x50(%rsp)
mov    %rdx,%r13
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x61a6>
mov    %rax,%rbp
mov    0x18(%rsp),%r9
mov    0x220(%r9),%rax
mov    %rax,0x0(%rbp)
mov    %rbx,0x8(%rbp)
mov    %rbp,0x220(%r9)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x53eb>
mov    %rcx,%rax
dec    %rax
mov    %rax,%rsi
imul   %r12,%rsi
lea    (%rsi,%rbp,1),%rdi
add    $0x40,%rdi
mov    0x10(%r9,%rbx,8),%r8
mov    %r8,0x40(%rbp,%rsi,1)
mov    %rdi,0x10(%r9,%rbx,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x53c5>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x54a0>
mov    %r9,%r12
lea    -0x4(%rax),%rcx
shr    $0x4,%r15d
imul   %r15,%rcx
shl    $0x4,%rcx
add    $0x40,%rcx
mov    %r15d,%edx
shl    $0x6,%edx
neg    %rdx
lea    -0x3(%rax),%rsi
imul   %r15,%rsi
shl    $0x4,%rsi
add    $0x40,%rsi
lea    -0x2(%rax),%rdi
imul   %r15,%rdi
shl    $0x4,%rdi
add    $0x40,%rdi
lea    -0x1(%rax),%r8
imul   %r15,%r8
shl    $0x4,%r8
add    $0x40,%r8
mov    %rbp,%r9
lea    (%r9,%r8,1),%r10
mov    0x10(%r12,%rbx,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x10(%r12,%rbx,8)
lea    (%r9,%rdi,1),%r10
mov    0x10(%r12,%rbx,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%r12,%rbx,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%r12,%rbx,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%r12,%rbx,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%r12,%rbx,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%r12,%rbx,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x544b>
add    $0x40,%rbp
mov    0x28(%rsp),%rbx
mov    0x50(%rsp),%r8
mov    %rbp,%rdi
xor    %esi,%esi
mov    %r8,%rdx
call   *0xc7b04(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x38(%rsp),%r15
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x54ec>
mov    %rdx,%r13
mov    $0x10,%esi
mov    %r8,%rdi
mov    %r8,%r12
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rbp
test   %rax,%rax
mov    0x28(%rsp),%rbx
mov    0x38(%rsp),%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6216>
mov    %r13,%r9
test   %r13,%r13
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x550f>
mov    %r9,%rdx
imul   %r14,%rdx
lea    0x58(%rsp),%rsi
mov    %rbp,%rdi
mov    %r9,%r12
call   *0xc79c4(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r12,%r9
mov    0x30(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5bd0>
lea    -0x2a2892(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x8be1>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
mov    0xc0(%rsp),%r8
jmp    *%rcx
mov    0x8(%rsp),%rax
mov    %al,0x0(%rbp,%r9,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c32>
mov    0x8(%rsp),%rax
cmp    %al,0x1(%rdi)
adc    $0x0,%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2dea>
mov    0x8(%rsp),%rcx
cmp    %cl,0x1(%rdi)
adc    $0x0,%rdx
xor    %eax,%eax
cmp    %cl,0x2(%rdi)
setb   %al
cmp    %cl,0x3(%rdi)
adc    %rax,%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2dea>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6201>
mov    %rax,%r14
mov    0x18(%rsp),%r8
mov    0x220(%r8),%rcx
mov    %rcx,(%r14)
movq   $0x1,0x8(%r14)
mov    %r14,0x220(%r8)
lea    0xfc0(%rax),%rcx
mov    $0x41,%edx
mov    0x18(%r8),%rsi
mov    %rsi,(%rcx)
mov    %rcx,0x18(%r8)
lea    -0x40(%rcx),%rsi
mov    0x18(%r8),%rdi
mov    %rdi,-0x40(%rcx)
mov    %rsi,0x18(%r8)
add    $0xffffffffffffff80,%rcx
add    $0xfffffffffffffffe,%rdx
cmp    $0x3,%rdx
jg     ADDR <expanse_trie::mutate::insert_with_path_flat+0x55bb>
add    $0x40,%r14
pxor   %xmm0,%xmm0
movdqa %xmm0,0x70(%rax)
movdqa %xmm0,0x60(%rax)
movdqa %xmm0,0x50(%rax)
movdqa %xmm0,0x40(%rax)
mov    0x68(%rsp),%rcx
test   %rcx,%rcx
mov    0x30(%rsp),%r8
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x563e>
mov    0x60(%rsp),%rax
lea    (%rax,%rcx,8),%rdx
nopl   0x0(%rax,%rax,1)
mov    (%rax),%rcx
mov    $0x1,%esi
shl    %cl,%rsi
add    $0x8,%rax
shr    $0x3,%ecx
and    $0x18,%ecx
or     %rsi,(%r14,%rcx,1)
cmp    %rdx,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5620>
movl   $0x0,0x83(%rsp)
movl   $0x0,0x80(%rsp)
mov    0x20(%rsp),%rdi
mov    %r14,(%rdi)
mov    0x80(%rsp),%eax
mov    0x83(%rsp),%ecx
mov    0x98(%rsp),%rdx
mov    %ecx,0x3(%rdx)
mov    %eax,(%rdx)
movb   $0xc,0xf(%rdi)
movzbl 0x68(%rsp),%eax
dec    %al
mov    %al,0x8(%rdi)
cmpq   $0x0,0x68(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x63cf>
mov    0x60(%rsp),%rax
mov    (%rax),%rcx
movzbl %r8b,%edx
mov    %rdi,%rbp
mov    $0x1,%esi
call   *0xc7be5(%rip)        # ADDR <_DYNAMIC+0x5d8>
cmpb   $0x1,0x30(%rsp)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x56f5>
mov    0x8(%rsp),%rax
shr    $0x8,%rax
mov    %rax,0x40(%r12)
mov    %r14,0x58(%r12)
movq   $0x0,0x60(%r12)
movzwl 0x68(%rsp),%eax
mov    %ax,0x70(%r12)
mov    %rbp,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
cmpq   $0x0,0x10(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6130>
imul   %rbx,%r15
lea    0xc(%r15),%rax
and    $0xfffffffffffffff0,%rax
mov    0x18(%rsp),%rcx
sub    %rax,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x177,%r15
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x576b>
lea    -0x29ccca(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%r15,%rax,1),%edi
cmp    $0xff,%rdi
mov    0x40(%rsp),%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5783>
cmp    $0x3e,%dil
mov    0x38(%rsp),%rcx
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63b1>
mov    0x18(%rsp),%rdx
mov    0x10(%rdx,%rdi,8),%rax
mov    0x10(%rsp),%rsi
mov    %rax,(%rsi)
mov    %rsi,0x10(%rdx,%rdi,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5793>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r15
mov    0x40(%rsp),%rbx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6167>
mov    0x10(%rsp),%rdi
call   *0xc775a(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x38(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5831>
shl    $0x4,%rcx
lea    (%rsp,%rcx,1),%rax
add    $0xc8,%rax
lea    0xc8(%rsp),%rcx
lea    0xc4d6f(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x57d3>
nopl   0x0(%rax,%rax,1)
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5831>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5805>
mov    0x8(%rdi),%r8
lea    -0x29eea3(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%r9
mov    %r9,%r10
and    %r8,%r10
inc    %r10
xor    %r8,%r10
and    %r9,%r10
xor    %r8,%r10
mov    %r10,0x8(%rdi)
mov    (%rbx),%r8
lea    -0x1(%r8),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x57ca>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x57c0>
mov    $0x8,%esi
call   *0xc7694(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e9>
cmpq   $0x0,0x58(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5844>
mov    0x60(%rsp),%rdi
call   *0xc76a4(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    $0x1,%al
and    $0x1,%al
add    $0x148,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
mov    %ecx,0x88(%rsp)
shr    $0x20,%rcx
mov    %cl,0x8c(%rsp)
mov    %eax,0x8d(%rsp)
shr    $0x20,%rax
mov    %al,0x91(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x595e>
mov    %cx,0x88(%rsp)
shr    $0x10,%ecx
mov    %cl,0x8a(%rsp)
mov    %ax,0x8b(%rsp)
shr    $0x10,%eax
mov    %al,0x8d(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x595e>
mov    %ecx,0x88(%rsp)
mov    %eax,0x8c(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x595e>
mov    %rcx,0x88(%rsp)
xor    %eax,%eax
mov    0x40(%rsp),%r11
mov    0x38(%rsp),%rsi
mov    0x20(%rsp),%rdi
mov    0x28(%rsp),%rbx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x595e>
mov    %cx,0x88(%rsp)
mov    %ax,0x8a(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x595e>
mov    %ecx,0x88(%rsp)
shr    $0x20,%rcx
mov    %cx,0x8c(%rsp)
mov    %eax,0x8e(%rsp)
shr    $0x20,%rax
mov    %ax,0x92(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x595e>
mov    %ecx,0x88(%rsp)
mov    %rcx,%rdx
shr    $0x20,%rdx
mov    %dx,0x8c(%rsp)
shr    $0x30,%rcx
mov    %cl,0x8e(%rsp)
mov    %eax,0x8f(%rsp)
mov    %rax,%rcx
shr    $0x20,%rcx
mov    %cx,0x93(%rsp)
shr    $0x30,%rax
mov    0x88(%rsp),%rcx
mov    %rcx,(%rdi)
lea    0x90(%rsp),%rdx
mov    (%rdx),%ecx
mov    %ecx,0x8(%rdi)
movzbl 0x4(%rdx),%ecx
mov    %cl,0xc(%rdi)
mov    %al,0xd(%rdi)
movb   $0x0,0xe(%rdi)
lea    0x1(%rbp),%eax
mov    %al,0xf(%rdi)
mov    $0x1,%al
test   %rsi,%rsi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
shl    $0x4,%rsi
lea    (%rsp,%rsi,1),%rcx
add    $0xc8,%rcx
lea    0xc8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x59c6>
xchg   %ax,%ax
mov    %sil,0x48(%rbx,%rdi,1)
incq   0x50(%rbx)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x59f8>
mov    0x8(%rdi),%r8
lea    -0x29f096(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%r9
mov    %r9,%r10
and    %r8,%r10
inc    %r10
xor    %r8,%r10
and    %r9,%r10
xor    %r8,%r10
mov    %r10,0x8(%rdi)
mov    (%r11),%r8
lea    -0x1(%r8),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x59b9>
mov    %rdi,(%rbx,%r8,8)
mov    0x50(%rbx),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x59b0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bca>
lea    (%rdx,%rdx,4),%rax
mov    %r13d,0xb0(%rsp,%rax,1)
shr    $0x20,%r13
mov    %r13b,0xb4(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5aaf>
lea    (%rdx,%rdx,2),%rax
mov    %r13w,0xb0(%rsp,%rax,1)
shr    $0x10,%r13d
mov    %r13b,0xb2(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5aaf>
mov    %r13d,0xb0(%rsp,%rdx,4)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5aaf>
mov    0x8(%rsp),%rax
mov    %rax,0xb0(%rsp,%rdx,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5aaf>
mov    %r13w,0xb0(%rsp,%rdx,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5aaf>
lea    (%rdx,%rdx,2),%rax
mov    %r13d,0xb0(%rsp,%rax,2)
shr    $0x20,%r13
mov    %r13w,0xb4(%rsp,%rax,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5aaf>
lea    (%rsp,%rdx,8),%rax
add    $0xb0,%rax
sub    %rdx,%rax
mov    %r13d,(%rax)
mov    %r13,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%r13
mov    %r13b,0x6(%rax)
add    $0x2,%r15b
mov    %r15d,%eax
mul    %r14b
cmp    $0x10,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61b6>
mov    0xb0(%rsp),%rax
mov    0x20(%rsp),%rdx
mov    %rax,(%rdx)
lea    0xb8(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x3(%rdi)
mov    %eax,(%rdi)
and    $0x70,%bpl
or     %sil,%bpl
mov    %bpl,0xf(%rdx)
cmpq   $0x0,0x38(%rsp)
mov    0x10(%rsp),%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    0x38(%rsp),%rcx
shl    $0x4,%rcx
add    %rsp,%rcx
add    $0xc8,%rcx
lea    0xc8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5b32>
mov    %sil,0x48(%r9,%rdi,1)
incq   0x50(%r9)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5b64>
mov    0x8(%rdi),%r8
lea    -0x29f202(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%r9
mov    %r9,%r10
and    %r8,%r10
inc    %r10
xor    %r8,%r10
and    %r9,%r10
xor    %r8,%r10
mov    %r10,0x8(%rdi)
mov    0x40(%rsp),%r8
mov    (%r8),%r8
lea    -0x1(%r8),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5b25>
mov    0x28(%rsp),%r9
mov    %rdi,(%r9,%r8,8)
mov    0x50(%r9),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5b1c>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bca>
lea    (%r9,%r9,4),%rax
mov    0x8(%rsp),%r13
mov    %r13d,0x0(%rbp,%rax,1)
shr    $0x20,%r13
mov    %r13b,0x4(%rbp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c32>
lea    (%r9,%r9,2),%rax
mov    0x8(%rsp),%r13
mov    %r13w,0x0(%rbp,%rax,1)
shr    $0x10,%r13d
mov    %r13b,0x2(%rbp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c32>
mov    0x8(%rsp),%rax
mov    %eax,0x0(%rbp,%r9,4)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c32>
mov    0x8(%rsp),%rax
mov    %rax,0x0(%rbp,%r9,8)
mov    0xc0(%rsp),%r8
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c32>
mov    0x8(%rsp),%rax
mov    %ax,0x0(%rbp,%r9,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c32>
lea    (%r9,%r9,2),%rax
mov    0x8(%rsp),%r13
mov    %r13d,0x0(%rbp,%rax,2)
shr    $0x20,%r13
mov    %r13w,0x4(%rbp,%rax,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c32>
lea    0x0(,%r9,8),%rax
add    %rbp,%rax
sub    %r9,%rax
mov    0x8(%rsp),%rdx
mov    %edx,(%rax)
mov    %rdx,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%rdx
mov    %dl,0x6(%rax)
mov    %r8,%rdx
sub    %r9,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c66>
mov    %r9,%rax
imul   %r14,%rax
lea    (%rsp,%rax,1),%rsi
add    $0x58,%rsi
inc    %r9
imul   %r14,%r9
add    %rbp,%r9
imul   %r14,%rdx
mov    %r9,%rdi
mov    %r8,%r12
call   *0xc726d(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r12,%r8
lea    0x4(%r14),%eax
movl   $0x0,0x83(%rsp)
movl   $0x0,0x80(%rsp)
mov    0x20(%rsp),%rsi
mov    %rbp,(%rsi)
mov    0x80(%rsp),%ecx
mov    0x83(%rsp),%edx
mov    0x70(%rsp),%rdi
mov    %edx,0x3(%rdi)
mov    %ecx,(%rdi)
mov    %al,0xf(%rsi)
mov    0x8(%rsi),%rax
xor    %rax,%r8
lea    -0x29f369(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rcx,%r14,8),%r8
xor    %rax,%r8
mov    %r8,0x8(%rsi)
test   %r15,%r15
mov    0x10(%rsp),%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    0x38(%rsp),%rcx
shl    $0x4,%rcx
add    %rsp,%rcx
add    $0xc8,%rcx
lea    0xc8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5cfd>
nopw   0x0(%rax,%rax,1)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5d2f>
mov    0x8(%rdi),%r8
lea    -0x29f3cd(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%r9
mov    %r9,%r10
and    %r8,%r10
inc    %r10
xor    %r8,%r10
and    %r9,%r10
xor    %r8,%r10
mov    %r10,0x8(%rdi)
mov    0x40(%rsp),%r8
mov    (%r8),%r8
lea    -0x1(%r8),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5cf0>
mov    %rdi,(%rbx,%r8,8)
mov    0x50(%rbx),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bca>
mov    %sil,0x48(%rbx,%rdi,1)
incq   0x50(%rbx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5cf0>
lea    (%r15,%r15,2),%rax
mov    %r8d,0x0(%rbp,%rax,2)
shr    $0x20,%r8
mov    %r8w,0x4(%rbp,%rax,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5dc7>
mov    %r8d,0x0(%rbp,%r15,4)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5dc7>
lea    (%r15,%r15,4),%rax
mov    %r8d,0x0(%rbp,%rax,1)
shr    $0x20,%r8
mov    %r8b,0x4(%rbp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5dc7>
lea    (%r15,%r15,2),%rax
mov    %r8w,0x0(%rbp,%rax,1)
shr    $0x10,%r8d
mov    %r8b,0x2(%rbp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5dc7>
lea    0x0(,%r15,8),%rax
add    %rbp,%rax
sub    %r15,%rax
mov    %r8d,(%rax)
mov    %r8,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%r8
mov    %r8b,0x6(%rax)
mov    0x40(%rsp),%r11
mov    0x0(%r13),%rax
mov    0xa0(%rsp),%r14
xor    %rax,%r14
and    %rsi,%r14
xor    %rax,%r14
mov    %r14,0x0(%r13)
mov    (%r11),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6003>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fe1>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fe1>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5e48>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29f4e6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fe1>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5e88>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29f526(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fe1>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5ec8>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29f566(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fe1>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f08>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29f5a6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fe1>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f48>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29f5e6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fe1>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f88>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29f626(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fe1>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fc8>
mov    0x28(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29f666(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
mov    0x28(%rsp),%r12
mov    0x40(%rsp),%r11
mov    0x38(%rsp),%r9
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6369>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
mov    $0x1,%al
test   %r9,%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
shl    $0x4,%r9
lea    (%rsp,%r9,1),%rcx
add    $0xc8,%rcx
lea    0xc8(%rsp),%rdx
xor    %esi,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x603d>
nopl   0x0(%rax)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5846>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%r8d
cmp    $0x7,%r8
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6073>
mov    0x8(%rdi),%rsi
lea    -0x29f70e(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%r8,8),%r9
mov    %r9,%r10
and    %rsi,%r10
inc    %r10
xor    %rsi,%r10
and    %r9,%r10
xor    %rsi,%r10
mov    %r10,0x8(%rdi)
mov    (%r11),%rsi
lea    -0x1(%rsi),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6030>
mov    %rdi,(%r12,%rsi,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bca>
mov    %r8b,0x48(%r12,%rdi,1)
mov    0x50(%r12),%rsi
inc    %rsi
mov    %rsi,0x50(%r12)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6030>
lea    0xbdcbd(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xb78>
mov    $0x7,%esi
call   *0xc6e0a(%rip)        # ADDR <_DYNAMIC+0x208>
lea    -0x29f6a2(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc968>
lea    0xc4a04(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x78d8>
mov    $0x13,%esi
call   *0xc6fb9(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x2c2dee(%rip),%rdi        # ADDR <anon.ADDR.91.llvm.ADDR+0x260>
lea    0xc495b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7848>
mov    $0x8,%esi
call   *0xc6fa0(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x2c2dcf(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
lea    0xc4a7a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7980>
mov    $0x8,%esi
call   *0xc6f87(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0xc49a8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x78c0>
mov    %r13,%rdi
mov    %rbx,%rsi
call   *0xc6e44(%rip)        # ADDR <_DYNAMIC+0x2a0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e9>
lea    0xc49f0(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7920>
mov    0x70(%rsp),%rdi
call   *0xc6e2d(%rip)        # ADDR <_DYNAMIC+0x2a0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e9>
lea    -0x2c2e17(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
lea    0xc4a1a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7968>
mov    $0x8,%esi
call   *0xc6f3f(%rip)        # ADDR <_DYNAMIC+0x3d0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e9>
lea    -0x2c2e35(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
lea    0xc493c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x78a8>
mov    $0x8,%esi
call   *0xc6f21(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x29f8b9(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0xc411b(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0xbd2a4(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0xaf(%rsp),%rdx
mov    $0x11,%esi
call   *0xc6df1(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e9>
lea    0xbd495(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x440>
mov    $0x8,%esi
call   *0xc6d1a(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0xc6d82(%rip)        # ADDR <_DYNAMIC+0x280>
lea    -0x29f7df(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0xc4934(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7908>
mov    $0x12,%esi
call   *0xc6eb9(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0xbc25a(%rip),%rcx        # ADDR <anon.ADDR.892.llvm.ADDR>
mov    %rdx,%rsi
mov    $0x7,%edx
mov    %rbx,%rdi
call   *0xc6d21(%rip)        # ADDR <_DYNAMIC+0x250>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e9>
lea    0xbd43d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x440>
mov    $0x8,%esi
mov    %rax,%rdi
call   *0xc6cbf(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0xc6d27(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e9>
mov    $0x10,%edi
mov    %r12,%rsi
call   *0xc6d14(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0xbd465(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4a0>
mov    $0x8,%esi
call   *0xc6c8a(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xc4843(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7890>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xc6c75(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xc4816(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7878>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xc6c60(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xc4741(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x77b8>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xc6c4b(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x6,%esi
call   *0xc6c40(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e9>
lea    0xc474c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x77e8>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xc6c26(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xc474f(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7800>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0xc6c0f(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xc4708(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x77d0>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xc6bfa(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xc473b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7818>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xc6be5(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xc473e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7830>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0xc6bce(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xc4757(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7860>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xc6bb9(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xbd0fa(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
call   *0xc6ba7(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xbd118(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r12,%rdi
call   *0xc6b92(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xbd103(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r15,%rdi
call   *0xc6b7d(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xbd0be(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x627a>
lea    0xbd0dd(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %rbx,%rdi
call   *0xc6b57(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xc41d0(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0xc6b40(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x10,%edi
mov    %r13,%rsi
call   *0xc6baa(%rip)        # ADDR <_DYNAMIC+0x280>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0xc6b9a(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e9>
mov    %r14,%rdi
lea    0xbd35e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x518>
mov    $0x8,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63e3>
lea    0xbd050(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5826>
lea    0xbd06f(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
call   *0xc6d01(%rip)        # ADDR <_DYNAMIC+0x418>
lea    0xc4752(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7938>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63df>
lea    0xc4761(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7950>
xor    %edi,%edi
xor    %esi,%esi
call   *0xc6ad7(%rip)        # ADDR <_DYNAMIC+0x208>
ud2
mov    $0x10,%edi
mov    %r15,%rsi
call   *0xc6b3f(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63fd>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x63fd>
mov    %rax,%rbx
cmpq   $0x0,0x58(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6419>
mov    0xc0(%rsp),%rax
mov    (%rax),%rdi
call   *0xc6acf(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
int3
int3
int3
int3
int3
int3
int3
int3
int3
int3
int3
int3
int3
int3
int3
