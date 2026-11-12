push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x138,%rsp
mov    %r8,%r12
mov    %ecx,%r9d
mov    %rdx,%r13
mov    %rsi,%r15
mov    %rdi,0x10(%rsp)
lea    0x50(%r8),%rbx
lea    0x60(%rsp),%rax
mov    %rax,0x90(%rsp)
xor    %r11d,%r11d
mov    %r8,0x50(%rsp)
mov    %rbx,0x40(%rsp)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
nopl   0x0(%rax)
movzbl 0xf(%r15),%ebp
cmp    $0xc,%ebp
mov    %r9,(%rsp)
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xe25>
lea    -0x2912d5(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8b11>
movslq (%rcx,%rbp,4),%rax
add    %rcx,%rax
jmp    *%rax
cmp    $0x8,%r9b
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2bbf>
mov    0x10(%rsp),%rcx
lock addq $0x40,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
mov    0x18(%rcx),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x112f>
mov    (%rbx),%rax
mov    %rax,0x18(%rcx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x11d0>
lea    0x8(%r15),%rsi
mov    0x8(%r15),%r14
movzbl %r14b,%ebp
lea    0x1(%rbp),%r8
cmp    $0x1,%r9b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2573>
lea    -0x1(%r9),%eax
movzbl %al,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6121>
lea    -0x28d5cd(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
shl    $0x8,%rax
mov    %r14,%rcx
xor    %r13,%rcx
test   %rcx,%rax
mov    (%rsp),%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2573>
mov    %r11,%r14
mov    (%rsi),%eax
mov    0x3(%rsi),%ecx
mov    %ecx,0x5b(%rsp)
mov    %eax,0x58(%rsp)
movzbl %dl,%eax
inc    %edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6031>
mov    %r13,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x150>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1111>
mov    %r15,0x18(%rsp)
mov    (%r15),%r15
movzbl 0x74(%r15),%r10d
movzbl %r10b,%edi
mov    %r9d,%eax
sub    %dil,%al
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0xcb6>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6179>
lea    -0x28d66d(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x18(%rsp),%rcx
mov    0x8(%rcx),%r8
mov    %r8,%rcx
xor    %r13,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xcb6>
cmp    $0x7,%r10b
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x61b1>
lea    -0x28d6a4(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    (%rbx),%rax
test   %rax,%rax
mov    0x18(%rsp),%r15
mov    0x50(%rsp),%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f0>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3cf>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3cf>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x24d>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28d6fb(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3cf>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x28d>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28d73b(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3cf>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2cd>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28d77b(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3cf>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x30d>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28d7bb(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3cf>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x34d>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28d7fb(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3cf>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x389>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28d837(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3cf>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3c5>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28d873(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %r11,%r14
lea    0x8(%r15),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0x5b(%rsp)
mov    %ecx,0x58(%rsp)
mov    (%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
nopl   (%rax)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6031>
mov    %r13,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x420>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1111>
mov    (%r15),%r14
movzbl 0x5(%r14),%ebp
mov    %r9d,%eax
sub    %bpl,%al
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0xd66>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6179>
movzbl %bpl,%edi
lea    -0x28d938(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x8(%r15),%r8
mov    %r8,%rcx
xor    %r13,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xd66>
cmp    $0x7,%bpl
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x61b1>
lea    -0x28d96a(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6ac>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x68b>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x68b>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x509>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28d9b7(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x68b>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x549>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28d9f7(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x68b>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x589>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28da37(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x68b>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c9>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28da77(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x68b>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x609>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28dab7(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x68b>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x645>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28daf3(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x68b>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x681>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28db2f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %r11,%r14
lea    0x8(%r15),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0x5b(%rsp)
mov    %ecx,0x58(%rsp)
mov    (%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
nopl   0x0(%rax)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6031>
mov    %r13,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6e0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1111>
cmp    $0x8,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61c3>
mov    (%r15),%rax
mov    %r11,%rcx
shl    $0x4,%rcx
mov    %r15,0xb8(%rsp,%rcx,1)
mov    %r9b,0xc0(%rsp,%rcx,1)
dec    %r9b
lea    0x0(,%r9,8),%ecx
mov    %r13,%rdx
shr    %cl,%rdx
movzbl %dl,%ecx
inc    %r11
shl    $0x4,%ecx
lea    (%rax,%rcx,1),%r15
add    $0x40,%r15
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    (%r15),%rdx
movzbl 0x5(%rdx),%esi
mov    %r9d,%eax
sub    %sil,%al
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0xdc0>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6179>
movzbl %sil,%edi
lea    -0x28dc42(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x8(%r15),%r8
mov    %r8,%rcx
xor    %r13,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xdc0>
cmp    $0x7,%sil
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x61b1>
lea    -0x28dc74(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9b6>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x995>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x995>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x813>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28dcc1(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x995>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x853>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28dd01(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x995>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x893>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28dd41(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x995>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x8d3>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28dd81(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x995>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x913>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ddc1(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x995>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x94f>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ddfd(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x995>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x98b>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28de39(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %r11,%r14
lea    0x8(%r15),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0x5b(%rsp)
mov    %ecx,0x58(%rsp)
mov    (%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6031>
mov    %r13,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x9f0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1111>
cmp    $0x1,%r9b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x25d9>
lea    -0x1(%r9),%eax
movzbl %al,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6121>
lea    -0x28df00(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rcx
shl    $0x8,%rcx
mov    0x8(%r15),%rax
mov    %rax,%rdx
xor    %r13,%rdx
test   %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x25d9>
mov    (%rbx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc59>
mov    0x68(%r12),%rdx
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc38>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc38>
movzbl 0x49(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xab6>
mov    0x8(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x28df64(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x2,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc38>
movzbl 0x4a(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xaf6>
mov    0x10(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x28dfa4(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x3,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc38>
movzbl 0x4b(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xb36>
mov    0x18(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x28dfe4(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x4,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc38>
movzbl 0x4c(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xb76>
mov    0x20(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x28e024(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x5,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc38>
movzbl 0x4d(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xbb6>
mov    0x28(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x28e064(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x6,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc38>
movzbl 0x4e(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xbf2>
mov    0x30(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x28e0a0(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x7,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc38>
movzbl 0x4f(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc2e>
mov    0x38(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x28e0dc(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x8,%rcx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %r11,%r14
lea    0x8(%r15),%rcx
movzbl %al,%r8d
inc    %r8
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x5b(%rsp)
mov    %eax,0x58(%rsp)
mov    (%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
nopw   0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6031>
mov    %r13,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xc90>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1111>
lea    -0x1(%r10),%ebp
lea    0x0(,%rbp,8),%ecx
mov    %r13,%rbx
shr    %cl,%rbx
mov    %ebx,%r14d
shr    $0x5,%r14d
and    $0x7,%r14d
mov    (%r15,%r14,4),%eax
mov    $0x1,%edx
mov    %ebx,%ecx
shl    %cl,%edx
bt     %ebx,%eax
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1407>
cmp    $0x8,%r11
mov    0x40(%rsp),%rbx
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61d8>
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
add    0x20(%r15,%r14,8),%rax
mov    0x18(%rsp),%rdx
mov    %rdx,0xb8(%rsp,%rcx,1)
mov    %r10b,0xc0(%rsp,%rcx,1)
inc    %r11
mov    %ebp,%r9d
mov    %rax,%r15
mov    0x50(%rsp),%r12
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
lea    -0x1(%rbp),%r9d
lea    0x0(,%r9,8),%ecx
mov    %r13,%rdx
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
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1765>
cmp    $0x4,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1722>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a66>
cmp    %dl,0x8(%r14)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a66>
xor    %ecx,%ecx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a7d>
lea    -0x1(%rsi),%edi
lea    0x0(,%rdi,8),%ecx
mov    %r13,%rax
shr    %cl,%rax
movzbl 0x4(%rdx),%ecx
test   %ecx,%ecx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2353>
movzbl 0x8(%rdx),%r8d
cmp    %al,%r8b
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x198e>
xor    %r9d,%r9d
cmp    $0x7,%r11
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6235>
mov    %r11,%rax
shl    $0x4,%rax
mov    %r15,0xb8(%rsp,%rax,1)
mov    %sil,0xc0(%rsp,%rax,1)
inc    %r11
shl    $0x4,%r9d
lea    (%rdx,%r9,1),%r15
add    $0x10,%r15
mov    %edi,%r9d
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
cmp    $0x7f,%ebp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc8>
lea    -0x6(%rbp),%eax
cmp    $0x6,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x285f>
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x103f>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x101e>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x101e>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xe9c>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e34a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x101e>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xedc>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e38a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x101e>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xf1c>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e3ca(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x101e>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xf5c>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e40a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x101e>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xf9c>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e44a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x101e>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0xfd8>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e486(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x101e>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1014>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e4c2(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
lea    -0x4(%rbp),%eax
lea    0x8(%r15),%rcx
mov    %rcx,0x18(%rsp)
movzbl %al,%ebx
mov    %r15,%r12
mov    0x8(%r15),%rax
lea    -0x28e524(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rbx,8),%rsi
mov    %rsi,%r15
and    %rax,%r15
lea    0x1(%r15),%r8
cmp    %bl,%r9b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f5d>
mov    %r9d,%ecx
sub    %bl,%cl
movzbl %cl,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6121>
lea    -0x28e554(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rdi,8),%rdx
lea    0x0(,%rbx,8),%r14d
mov    %r14d,%ecx
shl    %cl,%rdx
xor    %r13,%rax
test   %rax,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f65>
mov    %r11,%r14
mov    0x18(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x5b(%rsp)
mov    %eax,0x58(%rsp)
mov    (%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
mov    0x40(%rsp),%rbx
mov    %r12,%r15
mov    0x50(%rsp),%r12
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6031>
mov    %r13,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0x58(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x10f0>
movzbl %dl,%edx
mov    0x10(%rsp),%rdi
mov    %r15,%rsi
mov    %eax,%ecx
call   ADDR <expanse_trie::mutate::wrap_skip_level::<false>>
mov    %r14,%r11
mov    (%rsp),%r9
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    %r11,%r14
movq   $0x0,0x58(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x58(%rsp),%rdi
call   *0xdcaa0(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6133>
mov    0x58(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6133>
mov    $0x1000,%edx
mov    %rbx,%rdi
xor    %esi,%esi
call   *0xdca8a(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x10(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%rbx)
movq   $0x1,0x8(%rbx)
mov    %rbx,0x220(%rdi)
lea    0xfc0(%rbx),%rax
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
jg     ADDR <expanse_trie::mutate::insert_with_path_flat+0x11a0>
add    $0x40,%rbx
mov    %r14,%r11
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movdqu %xmm0,0x30(%rbx)
movdqu %xmm0,0x20(%rbx)
movdqu %xmm0,0x10(%rbx)
movb   $0x8,0x5(%rbx)
movl   $0x0,0x5b(%rsp)
movl   $0x0,0x58(%rsp)
mov    %rbx,(%r15)
mov    0x58(%rsp),%eax
mov    0x5b(%rsp),%ecx
mov    %eax,0x8(%r15)
mov    %ecx,0xb(%r15)
movb   $0x1,0xf(%r15)
mov    0x40(%rsp),%rbx
mov    (%rbx),%rax
mov    $0x8,%r9b
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x127e>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e72c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x12be>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e76c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x12fe>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e7ac(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x133e>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e7ec(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x137e>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e82c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x13be>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e86c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x42>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28e8b0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x42>
movdqa (%r15),%xmm0
movdqa 0x10(%r15),%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x1,%xmm2
movdqa -0x2b3243(%rip),%xmm3        # ADDR <anon.ADDR.407.llvm.ADDR+0x10>
pand   %xmm3,%xmm2
psubb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
movdqa -0x2b2fa7(%rip),%xmm4        # ADDR <anon.ADDR.407.llvm.ADDR+0x2c0>
pand   %xmm4,%xmm2
psrlw  $0x2,%xmm1
pand   %xmm4,%xmm1
paddb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x4,%xmm2
paddb  %xmm1,%xmm2
movdqa -0x2b331d(%rip),%xmm5        # ADDR <anon.ADDR.283.llvm.ADDR+0xf6>
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
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x19d0>
cmp    %r10b,(%rsp)
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d45>
cmp    $0x8,%r10b
mov    0x40(%rsp),%rbx
mov    0x18(%rsp),%r15
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61b1>
mov    %r11,%r14
lea    -0x28e9ad(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%r8
and    0x8(%r15),%r8
mov    (%rbx),%rax
test   %rax,%rax
mov    0x50(%rsp),%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16f8>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16d7>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16d7>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1555>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ea03(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16d7>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1595>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ea43(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16d7>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x15d5>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ea83(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16d7>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1615>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28eac3(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16d7>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1655>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28eb03(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16d7>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1691>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28eb3f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16d7>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x16cd>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28eb7b(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
inc    %r8
inc    %r10b
mov    (%rsp),%rbp
movzbl %bpl,%ecx
movzbl %r10b,%edx
mov    0x10(%rsp),%rdi
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate::wrap_skip_level::<false>>
mov    %r14,%r11
mov    %rbp,%r9
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
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1765>
tzcnt  %esi,%ecx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1a7d>
cmp    $0x7,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1faa>
mov    %r11,%r14
mov    (%rbx),%rax
test   %rax,%rax
mov    0x50(%rsp),%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x197c>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x195b>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x195b>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x17d9>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ec87(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x195b>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1819>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ecc7(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x195b>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1859>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ed07(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x195b>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1899>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ed47(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x195b>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x18d9>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28ed87(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x195b>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1915>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28edc3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x195b>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1951>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28edff(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    0x10(%rsp),%rdi
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate::upgrade_l7_to_b::<false>>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1123>
cmp    $0x1,%ecx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1abf>
cmp    %al,%r8b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d0a>
lea    0x10(%rdx),%r9
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2345>
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2353>
movzbl 0x9(%rdx),%r10d
mov    %r10b,0xa(%rdx)
mov    %r8b,0x9(%rdx)
movups 0x20(%rdx),%xmm0
movups %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2349>
movzwl 0x60(%r15,%r14,2),%esi
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
mov    %rax,0x8(%rsp)
mov    %rsi,(%rsp)
test   %rsi,%rsi
mov    %r10,0x28(%rsp)
mov    %rbp,0x30(%rsp)
mov    %r11,0x38(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2027>
mov    $0x2,%r12d
cmpl   $0x1,(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x202d>
cmpw   $0x10,(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x23b5>
cmpw   $0x2,(%rsp)
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x24b3>
mov    (%rsp),%rax
lea    0x4(%rax),%r12
and    $0x1c,%r12d
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x202d>
cmp    $0x2,%al
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f6f>
mov    $0x1,%ecx
cmp    %dl,0x9(%r14)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f6f>
cmp    $0x8,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6209>
mov    %r11,%rax
shl    $0x4,%rax
mov    %r15,0xb8(%rsp,%rax,1)
mov    %bpl,0xc0(%rsp,%rax,1)
cmp    $0x7,%ecx
mov    0x50(%rsp),%r12
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x621e>
inc    %r11
shl    $0x4,%ecx
lea    (%r14,%rcx,1),%r15
add    $0x10,%r15
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    $0x1,%r9d
cmp    %al,0x9(%rdx)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xded>
cmp    $0x2,%cl
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1997>
mov    $0x2,%r9d
cmp    %al,0xa(%rdx)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0xded>
cmp    $0x3,%ecx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1997>
mov    %r11,%r14
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cf8>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cd7>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cd7>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1b55>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f003(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cd7>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1b95>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f043(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cd7>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1bd5>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f083(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cd7>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1c15>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f0c3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cd7>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1c55>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f103(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cd7>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1c91>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f13f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1cd7>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ccd>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f17b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    0x10(%rsp),%rdi
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate::upgrade_l3_to_l7::<false>>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1123>
movzbl 0x9(%rdx),%r9d
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1d23>
mov    $0x2,%r8d
cmp    %al,%r9b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2356>
mov    $0x1,%r8d
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2356>
mov    %r9b,0xa(%rdx)
movdqu 0x20(%rdx),%xmm0
movdqu %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2356>
mov    %r11,%r14
mov    0x40(%rsp),%rbx
mov    (%rbx),%rax
test   %rax,%rax
mov    0x18(%rsp),%r15
mov    0x50(%rsp),%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f5d>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f3c>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f3c>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1dba>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f268(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f3c>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1dfa>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f2a8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f3c>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1e3a>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f2e8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f3c>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1e7a>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f328(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f3c>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1eba>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f368(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f3c>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1ef6>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f3a4(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f3c>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x1f32>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28f3e0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    0x10(%rsp),%rdi
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate::upgrade_b_to_u::<false>>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1123>
cmp    $0x3,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1faa>
cmp    %dl,0xa(%r14)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1faa>
cmp    $0x7,%r11
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6209>
mov    %r11,%rax
shl    $0x4,%rax
mov    %r15,0xb8(%rsp,%rax,1)
mov    %bpl,0xc0(%rsp,%rax,1)
mov    $0x2,%ecx
mov    0x50(%rsp),%r12
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x1aac>
mov    %r9,(%rsp)
lea    0x8(%r14),%rdi
lea    0x10(%r14),%rbx
movzbl %dl,%r8d
mov    $0x7,%edx
mov    %rbx,%rsi
mov    %rax,%rcx
mov    %r11,0x38(%rsp)
call   *0xdbec8(%rip)        # ADDR <_DYNAMIC+0x5a0>
mov    0x38(%rsp),%r11
incb   0x4(%r14)
or     %r12w,0x6(%r14)
cmp    $0x8,%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x625a>
mov    %r11,%rcx
shl    $0x4,%rcx
mov    %r15,0xb8(%rsp,%rcx,1)
mov    %bpl,0xc0(%rsp,%rcx,1)
cmp    $0x7,%eax
mov    0x50(%rsp),%r12
mov    (%rsp),%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x626f>
inc    %r11
shl    $0x4,%eax
add    %rax,%rbx
mov    %rbx,%r15
mov    0x40(%rsp),%rbx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    $0x1,%r12d
mov    %r12d,%edx
shl    $0x4,%edx
mov    0x10(%rsp),%rax
lock add %rdx,0x200(%rax)
lock incq 0x208(%rax)
lock incq 0x218(%rax)
lea    -0x28d401(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rax,1),%ebp
cmp    $0xff,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x222b>
cmp    $0x3d,%bpl
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x62ab>
mov    0x10(%rsp),%rcx
mov    0x10(%rcx,%rbp,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x208e>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2216>
cmp    $0x11,%r12d
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x222b>
mov    %rdx,0x88(%rsp)
movq   $0x0,0x58(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x58(%rsp),%rdi
call   *0xdbb32(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6133>
mov    0x58(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6133>
mov    $0x1000,%edx
xor    %esi,%esi
mov    %rdi,0x20(%rsp)
call   *0xdbb1a(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x20(%rsp),%r9
mov    0x10(%rsp),%r11
mov    0x220(%r11),%rax
mov    %rax,(%r9)
mov    %rbp,0x8(%r9)
mov    %r9,0x220(%r11)
mov    $0xfc0,%ax
xor    %edx,%edx
mov    0x88(%rsp),%r10
div    %r10w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2156>
mov    %rcx,%rax
nopl   0x0(%rax)
dec    %rax
mov    %rax,%rsi
imul   %r10,%rsi
lea    (%r9,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%r11,%rbp,8),%r8
mov    %r8,0x40(%r9,%rsi,1)
mov    %rdi,0x10(%r11,%rbp,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2130>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2205>
mov    %r12d,%r8d
shl    $0x4,%r8d
lea    -0x4(%rax),%rcx
imul   %r8,%rcx
add    $0x40,%rcx
shl    $0x6,%r12d
neg    %r12
lea    -0x3(%rax),%rdx
imul   %r8,%rdx
add    $0x40,%rdx
lea    -0x2(%rax),%rsi
imul   %r8,%rsi
add    $0x40,%rsi
lea    -0x1(%rax),%rdi
imul   %r8,%rdi
add    $0x40,%rdi
mov    0x20(%rsp),%r8
cs nopw 0x0(%rax,%rax,1)
lea    (%r8,%rdi,1),%r9
mov    0x10(%r11,%rbp,8),%r10
mov    %r10,(%r8,%rdi,1)
mov    %r9,0x10(%r11,%rbp,8)
lea    (%r8,%rsi,1),%r9
mov    0x10(%r11,%rbp,8),%r10
mov    %r10,(%r8,%rsi,1)
mov    %r9,0x10(%r11,%rbp,8)
lea    (%r8,%rdx,1),%r9
mov    0x10(%r11,%rbp,8),%r10
mov    %r10,(%r8,%rdx,1)
mov    %r9,0x10(%r11,%rbp,8)
add    $0xfffffffffffffffc,%rax
lea    (%r8,%rcx,1),%r9
mov    0x10(%r11,%rbp,8),%r10
mov    %r10,(%r8,%rcx,1)
mov    %r9,0x10(%r11,%rbp,8)
add    %r12,%r8
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x21b0>
mov    0x20(%rsp),%rdi
add    $0x40,%rdi
mov    0x88(%rsp),%rdx
xor    %esi,%esi
mov    %rdi,%rbp
call   *0xdb9df(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %rbp,%rdi
mov    0x30(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x224d>
mov    0x30(%rsp),%rbp
mov    $0x1,%esi
mov    %rdx,%rdi
mov    %rdx,%r12
call   *0xdb9b7(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%rdi
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x638d>
cmpq   $0x0,(%rsp)
mov    0x38(%rsp),%r11
mov    0x28(%rsp),%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2433>
mov    %r15,0xb0(%rsp)
mov    0x20(%r15,%r14,8),%rsi
mov    %rsi,0x88(%rsp)
mov    (%rsp),%r12
mov    0x8(%rsp),%rbp
shl    $0x4,%ebp
mov    %rbp,%rdx
mov    %rdi,%r15
call   *0xdb881(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r15,0x20(%rsp)
lea    (%r15,%rbp,1),%rdi
add    $0x10,%rdi
mov    0x88(%rsp),%r15
add    %r15,%rbp
mov    %r12,%rdx
sub    0x8(%rsp),%rdx
shl    $0x4,%rdx
mov    %rbp,%rsi
call   *0xdb854(%rip)        # ADDR <memcpy@GLIBC_2.14>
test   %r15,%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x605c>
mov    %r15,%rdx
mov    %r12,%rax
cmp    $0x3,%r12w
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x22e7>
cmpw   $0x11,(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x23d4>
mov    (%rsp),%rax
add    $0x3,%eax
and    $0x3c,%eax
shl    $0x4,%eax
mov    0x10(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
lea    -0x28d6b0(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rax,%rcx,1),%edi
cmp    $0xff,%rdi
mov    0x30(%rsp),%rbp
mov    0xb0(%rsp),%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x241b>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62c0>
mov    0x10(%rsp),%rcx
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2424>
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
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x61ed>
mov    %r11,%rax
shl    $0x4,%rax
mov    %r15,0xb8(%rsp,%rax,1)
lea    (%rdx,%r8,1),%r15
add    $0x10,%r15
mov    %sil,0xc0(%rsp,%rax,1)
inc    %r11
mov    %edi,%r9d
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
cmpw   $0x18,(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x24cf>
mov    $0x180,%edx
cmpl   $0x10,(%rsp)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x24fa>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2551>
mov    $0x180,%eax
cmpw   $0x19,(%rsp)
mov    0x30(%rsp),%rbp
mov    0xb0(%rsp),%r15
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2406>
mov    $0x200,%eax
cmpw   $0x21,(%rsp)
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2406>
mov    (%rsp),%rax
shl    $0x4,%eax
add    $0x30,%eax
and    $0xffffffc0,%eax
mov    0x10(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
mov    %rdx,%rdi
call   *0xdb704(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x38(%rsp),%r11
mov    0x28(%rsp),%rdx
mov    0x20(%rsp),%rdi
mov    0x8(%rsp),%rax
shl    $0x4,%eax
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rdi,%rax,1)
mov    %rdi,0x20(%r15,%r14,8)
mov    $0x1,%eax
mov    %ebx,%ecx
shl    %cl,%rax
mov    (%rsp),%rcx
inc    %ecx
mov    %cx,0x60(%r15,%r14,2)
shr    $0x3,%ebx
and    $0x18,%ebx
or     %rax,(%r15,%rbx,1)
cmp    $0x7,%r11
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6296>
mov    %r11,%rax
shl    $0x4,%rax
mov    0x18(%rsp),%rcx
mov    %rcx,0xb8(%rsp,%rax,1)
mov    %dl,0xc0(%rsp,%rax,1)
inc    %r11
mov    0x8(%rsp),%rax
shl    $0x4,%eax
add    0x20(%r15,%r14,8),%rax
mov    %ebp,%r9d
mov    %rax,%r15
mov    0x40(%rsp),%rbx
mov    0x50(%rsp),%r12
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x70>
mov    (%rsp),%rax
lea    0x4(%rax),%r12
add    $0x3,%eax
xor    %r12d,%eax
cmp    $0x4,%eax
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x24fa>
and    $0x1c,%r12d
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x202d>
cmpw   $0x20,(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x24e3>
mov    $0x200,%edx
cmpl   $0x18,(%rsp)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x24fa>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2551>
cmpl   $0x20,(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2544>
mov    (%rsp),%rcx
lea    0x4(%rcx),%eax
add    $0x3,%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2544>
mov    0x20(%r15,%r14,8),%r12
mov    0x8(%rsp),%rax
mov    %eax,%ebp
shl    $0x4,%ebp
lea    (%r12,%rbp,1),%rsi
lea    (%r12,%rbp,1),%rdi
add    $0x10,%rdi
mov    (%rsp),%rdx
sub    %rax,%rdx
shl    $0x4,%rdx
call   *0xdb6ba(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x28(%rsp),%rdx
mov    0x38(%rsp),%r11
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12,%rbp,1)
mov    0x30(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2449>
mov    (%rsp),%rdx
shl    $0x4,%edx
add    $0x40,%edx
and    $0xffffffc0,%edx
mov    0x10(%rsp),%rax
lock add %rdx,0x200(%rax)
lock incq 0x208(%rax)
lock incq 0x218(%rax)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2230>
movzbl %r13b,%ecx
mov    (%r15),%r10
movzbl (%r10,%rbp,1),%eax
mov    %r8,0x8(%rsp)
mov    %rcx,0x18(%rsp)
cmp    %rax,%rcx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2eb7>
cmp    $0x18,%ebp
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x306a>
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x35eb>
cmp    $0xe,%ebp
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3a34>
lea    0x5(%r14),%edx
and    $0x1c,%edx
cmp    $0x1,%ebp
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3a50>
movzwl 0xd(%r15),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%r15),%eax
mov    %eax,0x48(%rsp)
mov    $0x2,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x413d>
mov    (%r15),%rax
mov    %r13d,%ecx
shr    $0x3,%ecx
and    $0x18,%ecx
mov    (%rax,%rcx,1),%rdx
bts    %r13,%rdx
mov    %rdx,(%rax,%rcx,1)
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc8>
mov    0x8(%r15),%rcx
movzbl %cl,%eax
add    $0x2,%rax
cmp    $0x100,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3505>
cmp    $0x1,%r9b
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3505>
mov    (%r15),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60db>
mov    0x10(%rsp),%rdx
lock subq $0x40,0x200(%rdx)
lock decq 0x208(%rdx)
mov    0x18(%rdx),%rcx
mov    %rcx,(%rax)
mov    %rax,0x18(%rdx)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r15)
movb   $0x7f,0xf(%r15)
movb   $0xff,0x8(%r15)
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3554>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x283d>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x283d>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x26bb>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28fb69(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x283d>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x26fb>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28fba9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x283d>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x273b>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28fbe9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x283d>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x277b>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28fc29(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x283d>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x27bb>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28fc69(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x283d>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x27f7>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28fca5(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x283d>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2833>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x28fce1(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3554>
mov    %r15,0x18(%rsp)
test   %bpl,%bpl
js     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6043>
mov    %ebp,%r10d
shr    $0x4,%r10d
test   %r10b,%r10b
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6043>
mov    %ebp,%r15d
and    $0xf,%r15d
lea    0x1(%r15),%r14
mov    %r14,%rbx
imul   %r10,%rbx
cmp    $0xf,%rbx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6043>
mov    0x40(%rsp),%rax
mov    (%rax),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2aa9>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a83>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a83>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2901>
mov    0x8(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x28fdaf(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a83>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2941>
mov    0x10(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x28fdef(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a83>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2981>
mov    0x18(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x28fe2f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a83>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x29c1>
mov    0x20(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x28fe6f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a83>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a01>
mov    0x28(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x28feaf(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a83>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a3d>
mov    0x30(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x28feeb(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a83>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2a79>
mov    0x38(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x28ff27(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
mov    0x40(%rsp),%rax
movdqu %xmm0,(%rax)
movq   $0x0,0x10(%rax)
lea    0x0(,%r10,8),%ecx
mov    $0xffffffffffffffff,%rax
shl    %cl,%rax
not    %rax
and    %rax,%r13
test   %r15b,%r15b
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x34a1>
mov    %rbx,0x8(%rsp)
mov    0x18(%rsp),%r12
mov    (%r12),%rax
mov    0xb(%r12),%ecx
mov    %ecx,0x63(%rsp)
mov    0x8(%r12),%ecx
mov    %ecx,0x60(%rsp)
mov    %rax,0x58(%rsp)
movb   $0x0,0x67(%rsp)
movzbl %r10b,%edx
lea    0x58(%rsp),%rdi
mov    %r14,%rsi
mov    %rdx,(%rsp)
mov    %r13,%rcx
mov    %r11,0x38(%rsp)
mov    %r10,%rbx
call   ADDR <expanse_trie::leaf::locate>
mov    0x38(%rsp),%rsi
test   $0x1,%al
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
mov    %rax,0x28(%rsp)
add    $0x8,%r12
mov    $0xf,%al
movzbl %al,%eax
div    %bl
cmp    %al,%r14b
mov    %r14,%rdi
mov    %rbx,%r10
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3d70>
movdqu 0x58(%rsp),%xmm0
movdqa %xmm0,0xa0(%rsp)
cmp    %rdx,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2b93>
mov    %rdx,%r14
imul   %r10,%rdx
lea    0x1(%r14),%r8
imul   %r10,%r8
lea    0xd8b86(%rip),%r9        # ADDR <anon.ADDR.2.llvm.ADDR+0x78c0>
lea    0xa0(%rsp),%rdi
mov    $0x10,%esi
mov    0x8(%rsp),%rcx
mov    %r10,%rbx
call   ADDR <<[u8]>::copy_within::<core::ops::range::Range<usize>>>
mov    %r14,%rdx
mov    %rbx,%r10
lea    0x1(%r15),%rdi
mov    0x38(%rsp),%rsi
mov    (%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x59f9>
lea    -0x293da5(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x8b61>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %r13b,0xa0(%rsp,%rdx,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5a4c>
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2dc5>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2da4>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2da4>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2c22>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2900d0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2da4>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2c62>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x290110(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2da4>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ca2>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x290150(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2da4>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ce2>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x290190(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2da4>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d22>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2901d0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2da4>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d5e>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29020c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2da4>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2d9a>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x290248(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
cmp    $0x7,%r9b
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2de3>
lea    0x0(,%r9,8),%ecx
mov    $0xffffffffffffffff,%rax
shl    %cl,%rax
not    %rax
and    %rax,%r13
movl   $0x0,0x5b(%rsp)
movl   $0x0,0x58(%rsp)
shl    $0x4,%r9b
mov    %r13,(%r15)
mov    0x58(%rsp),%eax
mov    0x5b(%rsp),%ecx
mov    %eax,0x8(%r15)
mov    %ecx,0xb(%r15)
mov    %r9b,0xf(%r15)
mov    $0x1,%al
test   %r11,%r11
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
shl    $0x4,%r11
lea    (%rsp,%r11,1),%rcx
add    $0xb8,%rcx
lea    0xb8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e57>
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e89>
mov    0x8(%rdi),%r8
lea    -0x290337(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e4a>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2e40>
lea    0xd82bc(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
mov    $0x8,%esi
call   *0xdac49(%rip)        # ADDR <_DYNAMIC+0x208>
cmp    %eax,0x18(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc8>
mov    %r14d,%eax
and    $0xfc,%eax
cmp    $0xc,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x38fb>
movdqu (%r10),%xmm0
lea    -0x80(%r13),%eax
movzbl %al,%eax
movd   %eax,%xmm1
punpcklbw %xmm1,%xmm1
pshuflw $0x0,%xmm1,%xmm1
pshufd $0x44,%xmm1,%xmm1
pxor   -0x2b48b8(%rip),%xmm0        # ADDR <anon.ADDR.407.llvm.ADDR+0x470>
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
imul   $0x1010101,%eax,%ecx
shr    $0x18,%ecx
cmp    %rbp,%rcx
mov    %rcx,0x8(%rsp)
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2592>
cmp    %r13b,(%r10,%rcx,1)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2592>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc8>
lea    0x0(,%rbx,8),%r14d
mov    $0xffffffffffffffff,%rdx
mov    %r14d,%ecx
shl    %cl,%rdx
not    %rdx
and    %r13,%rdx
mov    (%r12),%rax
mov    %rax,0x8(%rsp)
test   %r8,%r8
mov    %rdx,0x28(%rsp)
mov    %r11,0x38(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2fc0>
lea    -0x2(%rbx),%eax
lea    -0x29415e(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8b99>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    0x8(%rsp),%rax
movzwl (%rax,%r15,2),%eax
mov    %r8,0x30(%rsp)
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc6>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ddc>
mov    0x18(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x4b(%rsp)
mov    %eax,0x48(%rsp)
mov    $0x1,%r13d
xor    %edi,%edi
movq   $0x0,0x30(%rsp)
imul   %rbx,%r13
lea    0xf(%r13),%rax
movabs $0x7ffffffffffffff0,%r15
and    %rax,%r15
mov    0x10(%rsp),%rax
lock add %r15,0x200(%rax)
lock incq 0x208(%rax)
lock incq 0x218(%rax)
lea    -0x28e3c6(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl 0x0(%r13,%rax,1),%ebp
cmp    $0xff,%rbp
mov    %r8,0x88(%rsp)
mov    %rsi,0x20(%rsp)
mov    %rdi,(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3473>
cmp    $0x3d,%bpl
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x62ab>
mov    0x10(%rsp),%rcx
mov    0x10(%rcx,%rbp,8),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x393c>
mov    (%r14),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x39db>
lea    0x4(%rbp),%eax
and    $0xfffffffc,%eax
cmp    $0x20,%rbp
mov    $0x20,%ebp
cmovae %rax,%rbp
movzwl 0xd(%r15),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%r15),%eax
mov    %eax,0x48(%rsp)
mov    (%rbx),%rax
test   %rax,%rax
mov    0x8(%rsp),%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32f1>
mov    %r12,%rdx
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32d3>
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32d3>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3102>
mov    %rsi,%r14
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2905a8(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    %r14,%rsi
mov    0x8(%rsp),%r14
cmp    $0x2,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32d3>
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x314e>
mov    %rsi,%r14
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2905f4(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    %r14,%rsi
mov    0x8(%rsp),%r14
cmp    $0x3,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32d3>
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x319a>
mov    %rsi,%r14
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x290640(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    %r14,%rsi
mov    0x8(%rsp),%r14
cmp    $0x4,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32d3>
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x31e6>
mov    %rsi,%r14
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29068c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    %r14,%rsi
mov    0x8(%rsp),%r14
cmp    $0x5,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32d3>
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3232>
mov    %rsi,%r14
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2906d8(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    %r14,%rsi
mov    0x8(%rsp),%r14
cmp    $0x6,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32d3>
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x327e>
mov    %rsi,%r14
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x290724(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    %r14,%rsi
mov    0x8(%rsp),%r14
cmp    $0x7,%rax
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32d3>
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x32c6>
mov    %rsi,%r14
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29076c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    %r14,%rsi
mov    0x8(%rsp),%r14
cmp    $0x8,%rax
mov    %r12,%rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%rdx)
movw   $0x0,0x70(%rdx)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
mov    %rsi,0x30(%rsp)
mov    %r10,0x28(%rsp)
mov    %r11,0x38(%rsp)
lea    0x58(%rsp),%rdi
mov    %r15,%rsi
mov    $0x1,%edx
mov    %r8,%rcx
call   *0xdabfa(%rip)        # ADDR <_DYNAMIC+0x618>
mov    0x68(%rsp),%rbx
cmp    %r14,%rbx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x608e>
cmp    0x58(%rsp),%rbx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3336>
lea    0x58(%rsp),%rdi
call   *0xdb10a(%rip)        # ADDR <_DYNAMIC+0xb48>
mov    %r14,%rax
lea    0x0(,%r14,8),%r14
add    0x60(%rsp),%r14
mov    %rbx,%rdx
sub    %rax,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x335f>
lea    0x8(%r14),%rdi
shl    $0x3,%rdx
mov    %r14,%rsi
call   *0xda881(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    0x1(%rbx),%rax
mov    0x18(%rsp),%rcx
mov    %rcx,(%r14)
mov    %rax,0x68(%rsp)
mov    (%rsp),%r14
cmp    $0x2,%r14b
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x343c>
lea    -0x1(%r14),%esi
movzbl %sil,%edi
dec    %rdi
xor    %ecx,%ecx
lea    0xcfc7e(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%sil
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6202>
mov    %rcx,%r8
shl    $0x8,%r8
movzbl 0x9(%r15,%rdi,1),%ecx
or     %r8,%rcx
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x33a0>
shl    $0x8,%rcx
mov    0x60(%rsp),%rsi
mov    %rsi,%rdx
cmp    $0x3,%rbx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3420>
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
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x33f0>
cmp    %rdi,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x343c>
lea    (%rsi,%rax,8),%rax
data16 data16 cs nopw 0x0(%rax,%rax,1)
or     %rcx,(%rdx)
add    $0x8,%rdx
cmp    %rax,%rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3430>
mov    0x10(%rsp),%rcx
lock addq $0x40,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
mov    0x18(%rcx),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x360c>
mov    (%rbx),%rax
mov    %rax,0x18(%rcx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x36ad>
cmp    $0xf,%r13
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3c1d>
mov    $0x1,%esi
mov    %r13,%rdi
call   *0xda76d(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r14
test   %rax,%rax
mov    0x30(%rsp),%rbp
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x39ee>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6321>
mov    0x18(%rsp),%r8
and    (%r8),%rax
cmp    %r13,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc8>
mov    %r11,%rdi
mov    %rax,%rcx
cmova  %r13,%rcx
cmovb  %r13,%rax
movq   $0x0,0x7d(%rsp)
movq   $0x0,0x78(%rsp)
movzbl %r10b,%edx
dec    %edx
cmp    $0x6,%edx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5888>
lea    -0x2946c8(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0x8b7d>
movslq (%rsi,%rdx,4),%rdx
add    %rsi,%rdx
mov    0x40(%rsp),%r11
jmp    *%rdx
mov    %cl,0x78(%rsp)
mov    %al,0x79(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x58f4>
lea    0x1(%rcx),%edx
and    $0xffffffffffffff00,%rcx
movzbl %dl,%edx
or     %rcx,%rdx
mov    %rdx,0x8(%r15)
cmp    $0x1,%r9b
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3554>
shr    $0x8,%r13
mov    %r13,0x40(%r12)
mov    (%r15),%rcx
mov    %rcx,0x58(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x60(%r12)
mov    %ax,0x70(%r12)
mov    %r15,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
mov    $0x1,%al
test   %r11,%r11
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
shl    $0x4,%r11
lea    (%rsp,%r11,1),%rcx
add    $0xb8,%rcx
lea    0xb8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x358d>
nopl   0x0(%rax,%rax,1)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x35bf>
mov    0x8(%rdi),%r8
lea    -0x290a6d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3580>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ea5>
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3580>
movzwl 0xd(%r15),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%r15),%eax
mov    %eax,0x48(%rsp)
mov    $0x2,%edx
mov    $0x1,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x413d>
movq   $0x0,0x70(%rsp)
lea    0x70(%rsp),%rdi
mov    $0x40,%esi
mov    $0x1000,%edx
call   *0xda5c6(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x630f>
mov    0x70(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x630f>
mov    $0x1000,%edx
mov    %rbx,%rdi
xor    %esi,%esi
call   *0xda5b0(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x10(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%rbx)
movq   $0x1,0x8(%rbx)
mov    %rbx,0x220(%rdi)
lea    0xfc0(%rbx),%rax
mov    $0x41,%ecx
nopw   0x0(%rax,%rax,1)
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
jg     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3680>
add    $0x40,%rbx
pxor   %xmm0,%xmm0
movdqu %xmm0,0x30(%rbx)
movdqu %xmm0,0x20(%rbx)
movdqu %xmm0,0x10(%rbx)
movdqu %xmm0,(%rbx)
mov    0x68(%rsp),%rdx
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3744>
mov    0x60(%rsp),%rsi
lea    0x0(,%rdx,8),%rdi
add    $0xfffffffffffffff8,%rdi
mov    %rsi,%rax
test   $0x8,%dil
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3702>
mov    (%rsi),%rcx
mov    $0x1,%r8d
shl    %cl,%r8
lea    0x8(%rsi),%rax
shr    $0x3,%ecx
and    $0x18,%ecx
or     %r8,(%rbx,%rcx,1)
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3744>
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
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3710>
movl   $0x0,0x73(%rsp)
movl   $0x0,0x70(%rsp)
mov    %rbx,(%r15)
mov    0x70(%rsp),%eax
mov    0x73(%rsp),%ecx
mov    0x30(%rsp),%rdx
mov    %ecx,0x3(%rdx)
mov    %eax,(%rdx)
movb   $0xc,0xf(%r15)
movzbl 0x68(%rsp),%eax
dec    %al
mov    %al,0x8(%r15)
cmp    $0x2,%r14b
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x379f>
add    $0x9,%r15
movzbl %r14b,%edx
dec    %rdx
lea    0x48(%rsp),%rsi
mov    %r15,%rdi
call   *0xda378(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x28(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x37e8>
cmp    $0x1,%r14b
mov    0x28(%rsp),%rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x37e8>
shr    $0x8,%r13
mov    %r13,0x40(%r12)
mov    %rbx,0x58(%r12)
movq   $0x0,0x60(%r12)
movzwl 0x68(%rsp),%eax
mov    %ax,0x70(%r12)
mov    %r15,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
lea    0xc(%rbp),%eax
and    $0xfffffff0,%eax
mov    0x10(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
lea    -0x28ebb4(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl 0x0(%rbp,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3848>
cmp    $0x3e,%dil
mov    0x40(%rsp),%r11
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62d2>
mov    0x10(%rsp),%rcx
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
mov    0x38(%rsp),%rax
test   %rax,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3864>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5815>
mov    %rdx,%rdi
call   *0xda2d7(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x40(%rsp),%r11
mov    0x38(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5815>
shl    $0x4,%rax
add    %rsp,%rax
add    $0xb8,%rax
lea    0xb8(%rsp),%rcx
lea    0xd78e8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x389d>
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5815>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x38cf>
mov    0x8(%rdi),%r8
lea    -0x290d7d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3890>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x624a>
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3890>
cmp    $0x4,%ebp
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3e0d>
xor    %ecx,%ecx
cmp    %r13b,(%r10)
setb   %cl
cmp    $0x1,%r8d
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f40>
cmp    $0x2,%r8d
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5564>
cmp    $0x3,%r8d
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5571>
xor    %eax,%eax
cmp    %r13b,0x1(%r10)
setb   %al
cmp    %r13b,0x2(%r10)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5586>
movq   $0x0,0x58(%rsp)
lea    0x58(%rsp),%rdi
mov    $0x40,%esi
mov    $0x1000,%edx
call   *0xda296(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6133>
mov    0x58(%rsp),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6133>
mov    $0x1000,%edx
mov    %r14,%rdi
xor    %esi,%esi
call   *0xda280(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x10(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%r14)
mov    %rbp,0x8(%r14)
mov    %r14,0x220(%rdi)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r15w
movzwl %ax,%eax
nopw   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %r15,%rcx
lea    (%r14,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x10(%rdi,%rbp,8),%rsi
mov    %rsi,0x40(%r14,%rcx,1)
mov    %rdx,0x10(%rdi,%rbp,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x39b0>
add    $0x40,%r14
mov    %r14,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0xda217(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x30(%rsp),%rbp
lea    -0x2(%rbx),%eax
cmp    $0x4,%eax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3c6e>
lea    -0x294b81(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8bdd>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
mov    0x8(%rsp),%r15
jmp    *%rax
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x485f>
lea    0x0(,%rbp,2),%rbx
mov    %r14,%rdi
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xda0e1(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4861>
xor    %eax,%eax
cmp    $0x17,%rbp
sete   %al
lea    0x18(,%rax,8),%rdx
mov    $0x18,%eax
cmp    $0xf,%rbp
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3a57>
lea    0x4(%r14),%eax
and    $0x1c,%eax
cmp    %eax,%edx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3d39>
mov    0x8(%rsp),%rax
cmp    %rbp,%rax
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3aa4>
mov    %rsi,0x30(%rsp)
lea    (%r10,%rax,1),%rsi
lea    (%r10,%rax,1),%rdi
inc    %rdi
mov    %r8,%rdx
sub    %rax,%rdx
mov    %r11,%r14
mov    %r10,%rbx
call   *0xda155(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x30(%rsp),%rsi
mov    %rbx,%r10
mov    %r14,%r11
mov    0x8(%rsp),%rax
lea    0x1(%rbp),%r8
mov    0x40(%rsp),%rbx
mov    %r13b,(%r10,%rax,1)
mov    $0xffffffffffffff00,%rax
and    (%rsi),%rax
or     %r8,%rax
mov    %rax,(%rsi)
cmpb   $0x1,(%rsp)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3e57>
shr    $0x8,%r13
mov    %r13,0x40(%r12)
movq   $0x0,0x58(%r12)
mov    %r10,0x60(%r12)
add    $0x2,%ebp
mov    %bp,0x70(%r12)
mov    %r15,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
mov    $0x1,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x407f>
lea    (%r15,%r15,2),%rax
mov    0x8(%rsp),%rdx
mov    (%rdx,%rax,2),%ecx
movzwl 0x4(%rdx,%rax,2),%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3b48>
mov    0x8(%rsp),%rax
mov    (%rax,%r15,4),%eax
mov    %r8,0x30(%rsp)
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc6>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ddc>
lea    (%r15,%r15,4),%rax
mov    0x8(%rsp),%rdx
mov    (%rdx,%rax,1),%ecx
movzbl 0x4(%rdx,%rax,1),%eax
mov    0x28(%rsp),%rdx
shl    $0x20,%rax
or     %rcx,%rax
mov    %r8,0x30(%rsp)
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc6>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ddc>
lea    (%r15,%r15,2),%rax
mov    0x8(%rsp),%rdx
movzwl (%rdx,%rax,1),%ecx
movzbl 0x2(%rdx,%rax,1),%eax
mov    0x28(%rsp),%rdx
shl    $0x10,%eax
or     %rcx,%rax
mov    %r8,0x30(%rsp)
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc6>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ddc>
mov    0x8(%rsp),%rax
lea    (%rax,%r15,8),%rax
sub    %r15,%rax
mov    (%rax),%ecx
movzwl 0x4(%rax),%edx
movzbl 0x6(%rax),%eax
shl    $0x20,%rdx
or     %rcx,%rdx
shl    $0x30,%rax
or     %rdx,%rax
mov    0x28(%rsp),%rdx
mov    %r8,0x30(%rsp)
cmp    %rax,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ddc>
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bcf>
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
lea    -0x2(%rbx),%eax
cmp    $0x4,%eax
mov    %rsi,0x20(%rsp)
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d7c>
lea    -0x294d93(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8bb1>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    0x8(%rsp),%rdi
mov    %r8,%rsi
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<2>>
lea    0x1(%r15),%r8
cmp    %r8,%rax
mov    %rax,0x30(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4dd2>
mov    0x8(%rsp),%rcx
movzwl (%rcx,%rax,2),%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d07>
movq   $0x0,0x58(%rsp)
lea    0x58(%rsp),%rdi
mov    $0x10,%esi
mov    %r13,%rdx
call   *0xd9fb7(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
mov    0x30(%rsp),%rbp
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6321>
mov    0x58(%rsp),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6321>
mov    %r14,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0xd9f9e(%rip)        # ADDR <memset@GLIBC_2.2.5>
lea    -0x2(%rbx),%eax
cmp    $0x4,%eax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x39fa>
test   %rbp,%rbp
mov    0x8(%rsp),%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x48d1>
lea    0x0(,%rbp,8),%rbx
sub    %rbp,%rbx
mov    %r14,%rdi
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xd9e7a(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x48d3>
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x47a5>
lea    0x0(,%rbp,2),%rax
lea    (%rax,%rax,2),%rbx
mov    %r14,%rdi
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xd9e51(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x47a7>
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x47e6>
lea    0x0(,%rbp,4),%rbx
mov    %r14,%rdi
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xd9e2c(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x47e8>
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x481e>
lea    0x0(,%rbp,4),%rbx
add    %rbp,%rbx
mov    %r14,%rdi
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xd9e04(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4820>
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4897>
lea    0x0(,%rbp,2),%rbx
add    %rbp,%rbx
mov    %r14,%rdi
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xd9ddc(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4899>
cmp    $0x10,%ebp
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4116>
lea    0x4(%r14),%esi
and    $0x1c,%esi
movzwl 0xd(%r15),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%r15),%eax
mov    %eax,0x48(%rsp)
cmp    $0xf,%ebp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x412d>
lea    0x5(%r14),%edx
and    $0x1c,%edx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x413d>
mov    %r12,0x8(%rsp)
lea    0x4(%rdi),%eax
and    $0xfffffffc,%eax
cmp    $0xf,%r15b
mov    $0x18,%r14d
cmovne %rax,%r14
imul   %r10,%r14
lea    0xc(%r14),%r15
mov    %r15d,%r12d
and    $0x3f0,%r12d
mov    0x10(%rsp),%rax
lock add %r12,0x200(%rax)
lock incq 0x208(%rax)
lock incq 0x218(%rax)
lea    -0x28f169(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%r14,%rax,1),%ebx
cmp    $0xff,%rbx
mov    %rdi,0x90(%rsp)
mov    %rdx,0x20(%rsp)
mov    %r10,0x30(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x54e0>
cmp    $0x3d,%bl
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x62e3>
mov    0x10(%rsp),%rcx
mov    0x10(%rcx,%rbx,8),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5383>
mov    0x0(%rbp),%rax
mov    %rax,0x10(%rcx,%rbx,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x54c1>
cmp    $0x8,%ebp
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c70>
lea    -0x80(%r13),%eax
movzbl %al,%eax
movd   %eax,%xmm0
punpcklbw %xmm0,%xmm0
pshuflw $0x0,%xmm0,%xmm0
movabs $0x8080808080808080,%rax
xor    (%r10),%rax
pshufd $0x44,%xmm0,%xmm0
movq   %rax,%xmm1
shufpd $0x2,-0x2b596a(%rip),%xmm1        # ADDR <anon.ADDR.407.llvm.ADDR+0x310>
pcmpgtb %xmm1,%xmm0
pmovmskb %xmm0,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f00>
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x407d>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x405c>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x405c>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3eba>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291368(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x405c>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3efa>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2913a8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x405c>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f3a>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2913e8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x405c>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3f7a>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291428(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x405c>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3fba>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291468(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x405c>
mov    %r11,%r9
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3ffd>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2914ab(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
mov    %r9,%r11
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x405c>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4045>
mov    0x50(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2914f3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
mov    0x50(%rsp),%r12
mov    0x40(%rsp),%rbx
mov    %r9,%r11
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
xor    %esi,%esi
mov    $0x1,%al
test   %r11,%r11
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
shl    $0x4,%r11
lea    (%rsp,%r11,1),%rcx
add    $0xb8,%rcx
lea    0xb8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x40c2>
mov    %r8b,0x48(%r12,%rdi,1)
mov    0x50(%r12),%rsi
inc    %rsi
mov    %rsi,0x50(%r12)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%r8d
cmp    $0x7,%r8
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x40f8>
mov    0x8(%rdi),%rsi
lea    -0x2915a3(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x40b5>
mov    %rdi,(%r12,%rsi,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x40a3>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ea5>
movzwl 0xd(%r15),%eax
mov    %ax,0x4c(%rsp)
mov    0x9(%r15),%eax
mov    %eax,0x48(%rsp)
mov    $0x18,%esi
xor    %eax,%eax
cmp    $0x17,%ebp
sete   %al
lea    0x18(,%rax,8),%rdx
lea    0xe(%rdx),%r8
mov    %r8d,%r12d
and    $0xfffffff0,%r12d
mov    0x10(%rsp),%rax
lock add %r12,0x200(%rax)
lock incq 0x208(%rax)
lock incq 0x218(%rax)
lea    -0x28f516(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rax,1),%ebx
cmp    $0xff,%rbx
mov    %r11,0x38(%rsp)
mov    %r10,0x28(%rsp)
mov    %rsi,0x30(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x41b5>
cmp    $0x3d,%bl
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x62e3>
mov    0x10(%rsp),%rcx
mov    0x10(%rcx,%rbx,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x41f1>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%rbx,8)
mov    0x50(%rsp),%rbx
mov    %rcx,%r12
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x436b>
cmp    $0xf,%edx
mov    %rdx,0x90(%rsp)
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4374>
mov    $0x1,%esi
mov    %rdx,%rdi
call   *0xd9a24(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,0x18(%rsp)
test   %rax,%rax
mov    0x50(%rsp),%rbx
mov    0x10(%rsp),%r12
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x43c7>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x635e>
mov    %r8,0x20(%rsp)
mov    %rdx,0x90(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6133>
mov    %rax,%r10
mov    0x10(%rsp),%rcx
mov    0x220(%rcx),%rax
mov    %rax,(%r10)
mov    %rbx,0x8(%r10)
mov    %r10,0x220(%rcx)
test   %r12,%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6351>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
mov    0x10(%rsp),%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4286>
mov    %rcx,%rax
nopl   (%rax)
dec    %rax
mov    %rax,%rsi
imul   %r12,%rsi
lea    (%r10,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%r9,%rbx,8),%r8
mov    %r8,0x40(%r10,%rsi,1)
mov    %rdi,0x10(%r9,%rbx,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4260>
mov    %r10,0x18(%rsp)
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
mov    %r9,%r12
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4355>
mov    0x20(%rsp),%r9
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
mov    0x18(%rsp),%r9
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4300>
mov    0x18(%rsp),%rdi
add    $0x40,%rdi
mov    0x50(%rsp),%rbx
mov    0x90(%rsp),%rdx
mov    %rdi,0x18(%rsp)
xor    %esi,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x43c1>
movq   $0x0,0x58(%rsp)
lea    0x58(%rsp),%rdi
mov    $0x10,%esi
call   *0xd9863(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
mov    0x50(%rsp),%rbx
mov    0x10(%rsp),%r12
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x635e>
mov    0x58(%rsp),%rax
mov    %rax,0x18(%rsp)
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x635e>
mov    0x18(%rsp),%rdi
xor    %esi,%esi
mov    0x90(%rsp),%rdx
call   *0xd9839(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x8(%rsp),%rax
test   %rax,%rax
mov    0x18(%rsp),%rsi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x43fd>
mov    %rsi,0x18(%rsp)
mov    0x18(%rsp),%rdi
mov    0x28(%rsp),%rsi
mov    %rax,%rdx
mov    %rax,0x8(%rsp)
call   *0xd971d(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%rsi
mov    0x8(%rsp),%rax
mov    %r13b,(%rsi,%rax,1)
cmp    %rbp,%rax
mov    0x28(%rsp),%r8
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x442d>
lea    (%rsi,%rax,1),%rdi
lea    (%r8,%rax,1),%rsi
inc    %rdi
lea    0x1(%rbp),%rdx
sub    %rax,%rdx
call   *0xd96ed(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%rsi
mov    0x28(%rsp),%r8
mov    %rsi,(%r15)
mov    %r14b,0x8(%r15)
mov    0x48(%rsp),%eax
mov    %eax,0x9(%r15)
movzwl 0x4c(%rsp),%eax
mov    %ax,0xd(%r15)
mov    $0xffffffffffffff00,%rcx
and    0x8(%r15),%rcx
lea    0x1(%rbp),%rax
or     %rax,%rcx
mov    %rcx,0x8(%r15)
mov    0x30(%rsp),%rcx
lea    0xf(%rcx),%eax
and    $0xfffffff0,%eax
lock sub %rax,0x200(%r12)
lock decq 0x208(%r12)
lea    -0x28f82a(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rcx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x44f7>
cmp    $0x3e,%dil
mov    (%rsp),%rcx
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62c0>
mov    0x10(%r12,%rdi,8),%rax
mov    %rax,(%r8)
mov    %r8,0x10(%r12,%rdi,8)
mov    0x38(%rsp),%r9
cmp    $0x1,%cl
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4513>
shr    $0x8,%r13
mov    %r13,0x40(%rbx)
movq   $0x0,0x58(%rbx)
mov    %rsi,0x60(%rbx)
add    $0x2,%ebp
mov    %bp,0x70(%rbx)
mov    %r15,(%rbx)
movb   $0x1,0x48(%rbx)
movq   $0x1,0x50(%rbx)
movq   $0x0,0x68(%rbx)
mov    $0x1,%edx
test   %r9,%r9
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4714>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5828>
mov    %r8,%rdi
call   *0xd9628(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x18(%rsp),%rsi
mov    0x38(%rsp),%r9
mov    (%rsp),%rcx
cmp    $0x1,%cl
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x44b2>
mov    0x40(%rsp),%rax
mov    (%rax),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4709>
mov    0x68(%rbx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46e6>
movq   $0x0,0x68(%rbx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46e6>
movzbl 0x49(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4576>
mov    0x8(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291a24(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46e6>
movzbl 0x4a(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x45b3>
mov    0x10(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291a61(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46e6>
movzbl 0x4b(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x45f0>
mov    0x18(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291a9e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46e6>
movzbl 0x4c(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x462d>
mov    0x20(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291adb(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46e6>
movzbl 0x4d(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x466a>
mov    0x28(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291b18(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46e6>
movzbl 0x4e(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46a3>
mov    0x30(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291b51(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46e6>
movzbl 0x4f(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x46dc>
mov    0x38(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291b8a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%rbx)
movw   $0x0,0x70(%rbx)
pxor   %xmm0,%xmm0
mov    0x40(%rsp),%rax
movdqu %xmm0,(%rax)
movq   $0x0,0x10(%rax)
xor    %edx,%edx
test   %r9,%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5828>
shl    $0x4,%r9
lea    (%rsp,%r9,1),%rax
add    $0xb8,%rax
lea    0xb8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x473d>
nopl   0x0(%rax)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5828>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4777>
mov    0x8(%rdi),%rdx
lea    -0x291c1d(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4730>
mov    %rdi,(%rbx,%rdx,8)
mov    0x50(%rbx),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ea5>
mov    %sil,0x48(%rbx,%rdi,1)
mov    0x50(%rbx),%rdx
inc    %rdx
mov    %rdx,0x50(%rbx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4730>
xor    %ebx,%ebx
mov    0x20(%rsp),%rsi
mov    0x28(%rsp),%rax
mov    %eax,(%r14,%rbx,1)
shr    $0x20,%rax
mov    %ax,0x4(%r14,%rbx,1)
mov    0x88(%rsp),%r8
mov    %r8,%rax
sub    %rbp,%rax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4932>
lea    (%r14,%rbx,1),%rdi
add    %r15,%rbx
add    $0x6,%rdi
add    %rax,%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x48cb>
xor    %ebx,%ebx
mov    0x20(%rsp),%rsi
mov    0x28(%rsp),%rax
mov    %eax,(%r14,%rbx,1)
mov    0x88(%rsp),%r8
mov    %r8,%rdx
sub    %rbp,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4932>
lea    (%r14,%rbx,1),%rdi
add    %r15,%rbx
add    $0x4,%rdi
shl    $0x2,%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x491d>
xor    %ebx,%ebx
mov    0x20(%rsp),%rsi
mov    0x28(%rsp),%rax
mov    %eax,(%r14,%rbx,1)
shr    $0x20,%rax
mov    %al,0x4(%r14,%rbx,1)
mov    0x88(%rsp),%r8
mov    %r8,%rax
sub    %rbp,%rax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4932>
lea    (%r14,%rbx,1),%rdi
add    %r15,%rbx
add    $0x5,%rdi
lea    (%rax,%rax,4),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x491d>
xor    %ebx,%ebx
mov    0x20(%rsp),%rsi
mov    0x28(%rsp),%rax
mov    %ax,(%r14,%rbx,1)
mov    0x88(%rsp),%r8
mov    %r8,%rdx
sub    %rbp,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4932>
lea    (%r14,%rbx,1),%rdi
add    %r15,%rbx
add    $0x2,%rdi
add    %rdx,%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x491d>
xor    %ebx,%ebx
mov    0x20(%rsp),%rsi
mov    0x28(%rsp),%rax
mov    %ax,(%r14,%rbx,1)
shr    $0x10,%eax
mov    %al,0x2(%r14,%rbx,1)
mov    0x88(%rsp),%r8
mov    %r8,%rax
sub    %rbp,%rax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4932>
lea    (%r14,%rbx,1),%rdi
add    %r15,%rbx
add    $0x3,%rdi
lea    (%rax,%rax,2),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x491d>
xor    %ebx,%ebx
mov    0x20(%rsp),%rsi
mov    0x28(%rsp),%rcx
mov    %ecx,(%r14,%rbx,1)
mov    %rcx,%rax
shr    $0x20,%rax
mov    %ax,0x4(%r14,%rbx,1)
shr    $0x30,%rcx
mov    %cl,0x6(%r14,%rbx,1)
mov    0x88(%rsp),%r8
mov    %r8,%rax
sub    %rbp,%rax
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4932>
lea    (%r14,%rbx,1),%rdi
add    %r15,%rbx
add    $0x7,%rdi
lea    0x0(,%rax,8),%rdx
sub    %rax,%rdx
mov    %rsi,%r13
mov    %rbx,%rsi
mov    %r8,%rbx
call   *0xd91e4(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%rsi
mov    %rbx,%r8
mov    %r14,(%r12)
mov    0x48(%rsp),%eax
mov    0x4b(%rsp),%ecx
mov    0x18(%rsp),%rdx
mov    %ecx,0x3(%rdx)
mov    %eax,(%rdx)
mov    0x8(%r12),%rax
xor    %rax,%r8
and    %rsi,%r8
xor    %rax,%r8
mov    %r8,0x8(%r12)
test   %r15,%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6075>
mov    (%rsp),%rdx
lea    0xf(%rdx),%rax
and    $0xfffffffffffffff0,%rax
mov    0x10(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
lea    -0x28fd36(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rax,1),%edi
cmp    $0xff,%rdi
mov    0x40(%rsp),%rbx
mov    0x50(%rsp),%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x49cc>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62c0>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%r15)
mov    %r15,0x10(%rcx,%rdi,8)
mov    0x38(%rsp),%r9
mov    (%rbx),%rax
test   %rax,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x49e6>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bce>
mov    %r15,%rdi
call   *0xd9153(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x38(%rsp),%r9
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4bce>
mov    0x68(%r14),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4baf>
movq   $0x0,0x68(%r14)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4baf>
movzbl 0x49(%r14),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4a39>
mov    0x8(%r14),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291ee7(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4baf>
movzbl 0x4a(%r14),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4a77>
mov    0x10(%r14),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291f25(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4baf>
movzbl 0x4b(%r14),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ab5>
mov    0x18(%r14),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291f63(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4baf>
movzbl 0x4c(%r14),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4af3>
mov    0x20(%r14),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291fa1(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4baf>
movzbl 0x4d(%r14),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b31>
mov    0x28(%r14),%rsi
mov    0x8(%rsi),%rdi
lea    -0x291fdf(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4baf>
movzbl 0x4e(%r14),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4b6b>
mov    0x30(%r14),%rsi
mov    0x8(%rsi),%rdi
lea    -0x292019(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4baf>
movzbl 0x4f(%r14),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ba5>
mov    0x38(%r14),%rsi
mov    0x8(%rsi),%rdi
lea    -0x292053(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%r14)
movw   $0x0,0x70(%r14)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rbx)
movq   $0x0,0x10(%rbx)
test   %r9,%r9
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5828>
shl    $0x4,%r9
lea    (%rsp,%r9,1),%rax
add    $0xb8,%rax
lea    0xb8(%rsp),%rcx
xor    %edx,%edx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c0d>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5828>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c42>
mov    0x8(%rdi),%rdx
lea    -0x2920ed(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c00>
mov    %rdi,(%r14,%rdx,8)
mov    0x50(%r14),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ea5>
mov    %sil,0x48(%r14,%rdi,1)
mov    0x50(%r14),%rdx
inc    %rdx
mov    %rdx,0x50(%r14)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c00>
xor    %ecx,%ecx
mov    %r8,%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c83>
mov    %rdi,%rcx
cmp    %rax,%rcx
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f40>
mov    %rcx,%rdi
add    %rax,%rcx
shr    $1,%rcx
movzbl (%r10,%rcx,1),%edx
cmp    %rdx,0x18(%rsp)
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c9b>
mov    %rcx,%rax
cmp    %edx,0x18(%rsp)
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c77>
inc    %rcx
cmp    %rax,%rcx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4c83>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f40>
mov    0x8(%rsp),%rdi
mov    %r8,%rsi
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<6>>
lea    0x1(%r15),%r8
cmp    %r8,%rax
mov    %rax,0x30(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4dd2>
lea    (%rax,%rax,2),%rax
mov    0x8(%rsp),%rdx
mov    (%rdx,%rax,2),%ecx
movzwl 0x4(%rdx,%rax,2),%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d41>
mov    0x8(%rsp),%rdi
mov    %r8,%rsi
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<4>>
lea    0x1(%r15),%r8
cmp    %r8,%rax
mov    %rax,0x30(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4dd2>
mov    0x8(%rsp),%rcx
mov    (%rcx,%rax,4),%eax
cmp    %rax,0x28(%rsp)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4dc0>
mov    0x8(%rsp),%rdi
mov    %r8,%rsi
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<5>>
lea    0x1(%r15),%r8
cmp    %r8,%rax
mov    %rax,0x30(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4dd2>
lea    (%rax,%rax,4),%rax
mov    0x8(%rsp),%rdx
mov    (%rdx,%rax,1),%ecx
movzbl 0x4(%rdx,%rax,1),%eax
shl    $0x20,%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4d77>
mov    0x8(%rsp),%rdi
mov    %r8,%rsi
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<3>>
lea    0x1(%r15),%r8
cmp    %r8,%rax
mov    %rax,0x30(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4dd2>
lea    (%rax,%rax,2),%rax
mov    0x8(%rsp),%rdx
movzwl (%rdx,%rax,1),%ecx
movzbl 0x2(%rdx,%rax,1),%eax
shl    $0x10,%eax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4dbb>
mov    0x8(%rsp),%rdi
mov    %r8,%rsi
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<7>>
lea    0x1(%r15),%r8
cmp    %r8,%rax
mov    %rax,0x30(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4dd2>
mov    0x8(%rsp),%rcx
lea    (%rcx,%rax,8),%rsi
sub    %rax,%rsi
mov    (%rsi),%ecx
movzwl 0x4(%rsi),%edx
movzbl 0x6(%rsi),%eax
shl    $0x20,%rdx
or     %rcx,%rdx
shl    $0x30,%rax
or     %rdx,%rax
cmp    0x28(%rsp),%rax
mov    0x38(%rsp),%r11
mov    0x20(%rsp),%rsi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x3bc8>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4ddc>
mov    0x38(%rsp),%r11
mov    0x20(%rsp),%rsi
cmp    $0x20,%r8
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4e21>
test   %r15,%r15
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5087>
cmp    $0xf,%r15
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x50a7>
lea    0x5(%r15),%rax
cmp    $0x3,%r8
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x50f3>
mov    %r8,%rdi
imul   %rbx,%rdi
mov    0x18(%rsp),%rdx
mov    (%rdx),%ecx
mov    0x3(%rdx),%edx
mov    %edx,0x4b(%rsp)
mov    %ecx,0x48(%rsp)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x51b1>
and    $0xfffffffffffffffc,%r15
add    $0x4,%r15
cmp    $0x20,%r8
cmove  %r8,%r15
mov    0x18(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x4b(%rsp)
mov    %eax,0x48(%rsp)
lea    0x58(%rsp),%rdi
mov    %r12,%rsi
mov    %ebx,%edx
mov    %r8,%rcx
call   *0xd90ba(%rip)        # ADDR <_DYNAMIC+0x618>
mov    0x68(%rsp),%rsi
cmp    0x30(%rsp),%rsi
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60a6>
mov    %rsi,0x20(%rsp)
cmp    0x58(%rsp),%rsi
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4e7d>
lea    0x58(%rsp),%rdi
call   *0xd95c3(%rip)        # ADDR <_DYNAMIC+0xb48>
mov    0x30(%rsp),%rax
lea    0x0(,%rax,8),%rcx
add    0x60(%rsp),%rcx
mov    0x20(%rsp),%rdx
sub    %rax,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4eb6>
lea    0x8(%rcx),%rdi
shl    $0x3,%rdx
mov    %rcx,0x30(%rsp)
mov    0x30(%rsp),%rsi
call   *0xd8d2f(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x30(%rsp),%rcx
mov    0x20(%rsp),%rax
lea    0x1(%rax),%r8
mov    0x28(%rsp),%rax
mov    %rax,(%rcx)
mov    %r8,0x68(%rsp)
mov    (%rsp),%r9
cmp    %bl,%r9b
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4fb1>
mov    %rbx,%rsi
xor    $0x7,%rsi
mov    %r9d,%eax
sub    %bl,%al
movzbl %al,%edi
dec    %rdi
lea    -0x4(%rbp),%eax
movzbl %al,%eax
lea    (%r12,%rax,1),%rcx
add    $0x7,%rcx
xor    %eax,%eax
lea    0xce10e(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    %rsi,%rdi
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x628b>
mov    %rax,%r9
shl    $0x8,%r9
movzbl 0x1(%rcx,%rdi,1),%eax
or     %r9,%rax
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4f10>
mov    %r14d,%ecx
shl    %cl,%rax
mov    0x60(%rsp),%rdx
mov    %rdx,%rcx
cmpq   $0x3,0x20(%rsp)
mov    (%rsp),%r9
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4f9c>
movabs $0x1ffffffffffffffc,%rsi
and    %r8,%rsi
lea    (%rdx,%rsi,8),%rcx
movq   %rax,%xmm0
pshufd $0x44,%xmm0,%xmm0
xor    %edi,%edi
data16 cs nopw 0x0(%rax,%rax,1)
movdqu (%rdx,%rdi,8),%xmm1
movdqu 0x10(%rdx,%rdi,8),%xmm2
por    %xmm0,%xmm1
por    %xmm0,%xmm2
movdqu %xmm1,(%rdx,%rdi,8)
movdqu %xmm2,0x10(%rdx,%rdi,8)
add    $0x4,%rdi
cmp    %rdi,%rsi
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4f70>
cmp    %rsi,%r8
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x4fac>
lea    (%rdx,%r8,8),%rdx
or     %rax,(%rcx)
add    $0x8,%rcx
cmp    %rdx,%rcx
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x4fa0>
mov    0x68(%rsp),%r8
mov    0x60(%rsp),%rax
cmp    $0x21,%r8
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5003>
mov    0x10(%rsp),%rdi
mov    %r12,%rsi
mov    %ebx,%edx
mov    %rax,%rcx
call   ADDR <expanse_trie::mutate::build_leaf::<false>>
mov    (%rsp),%rax
cmp    %bl,%al
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x56eb>
movzbl %al,%edx
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x615c>
sub    %rbx,%rdx
lea    (%rsp,%rbx,1),%rsi
add    $0x48,%rsi
mov    0x18(%rsp),%rdi
add    %rbx,%rdi
call   *0xd8b12(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x56eb>
mov    -0x8(%rax,%r8,8),%rdx
xor    (%rax),%rdx
cmp    $0x7,%r9b
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5029>
lea    0x0(,%r9,8),%ecx
mov    $0xffffffffffffffff,%rsi
shl    %cl,%rsi
not    %rsi
and    %rsi,%rdx
mov    $0x7f,%ecx
bsr    %rdx,%rcx
xor    $0x3f,%ecx
mov    0x10(%rsp),%rdx
lock addq $0x40,0x200(%rdx)
mov    $0x3f,%sil
lock incq 0x208(%rdx)
sub    %cl,%sil
lock incq 0x218(%rdx)
mov    %esi,%ebp
shr    $0x3,%bpl
mov    0x18(%rdx),%r14
test   %bpl,%bpl
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x50ba>
test   %r14,%r14
mov    %sil,0x30(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x520c>
mov    (%r14),%rax
mov    0x10(%rsp),%rcx
mov    %rax,0x18(%rcx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x527e>
mov    0x18(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x4b(%rsp)
mov    %eax,0x48(%rsp)
mov    $0x2,%r13d
mov    %rbx,%rdi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2fe3>
xor    %eax,%eax
cmp    $0x17,%r15
setae  %al
lea    0x18(,%rax,8),%rax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x50f6>
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x558e>
mov    (%r14),%rcx
mov    0x10(%rsp),%rdx
mov    %rcx,0x18(%rdx)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x30(%r14)
movdqu %xmm0,0x20(%r14)
movdqu %xmm0,0x10(%r14)
movdqu %xmm0,(%r14)
lea    (%rax,%r8,8),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5630>
and    $0x1c,%eax
mov    0x8(%rsp),%r13
xor    %ecx,%ecx
cmp    $0x19,%r8
setae  %cl
mov    %r15,%rdi
and    $0xfffffffffffffffc,%rdi
add    $0x4,%rdi
cmp    $0x11,%r8
lea    0x18(,%rcx,8),%rcx
mov    %rcx,%rdx
cmovb  %rdi,%rdx
cmp    %rdx,%rax
mov    0x30(%rsp),%rbp
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x517e>
mov    %r8,%rdx
mov    %r11,%r14
sub    %rbp,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x51c7>
mov    %rsi,%r12
mov    %rbp,%rsi
imul   %rbx,%rsi
add    %r13,%rsi
lea    0x1(%rbp),%rdi
imul   %rbx,%rdi
add    %r13,%rdi
imul   %rbx,%rdx
mov    %r8,%r15
call   *0xd8a80(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r12,%rsi
mov    %r15,%r8
lea    -0x2(%rbx),%eax
lea    -0x296308(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8bc5>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
mov    0x50(%rsp),%rbx
jmp    *%rax
mov    0x18(%rsp),%rdx
mov    (%rdx),%eax
mov    0x3(%rdx),%edx
mov    %edx,0x4b(%rsp)
mov    %eax,0x48(%rsp)
cmp    $0x11,%r8
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x51ef>
imul   %rbx,%rdi
mov    $0x18,%r13d
cmp    $0xe,%r15
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2fe3>
add    $0x5,%r15
mov    %r15,%rax
movabs $0x7ffffffffffffff0,%r13
or     $0xc,%r13
and    %rax,%r13
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2fe3>
lea    -0x2(%rbx),%eax
lea    -0x296369(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x8bc5>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
mov    0x50(%rsp),%rbx
jmp    *%rax
mov    0x28(%rsp),%rax
mov    %ax,0x0(%r13,%rbp,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5d72>
imul   %rbx,%rcx
xor    %eax,%eax
cmp    $0x17,%r15
setae  %al
lea    0x18(,%rax,8),%r13
mov    %rcx,%rdi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2fe3>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x618e>
mov    %rax,%r14
mov    0x10(%rsp),%rdi
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
jg     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5251>
add    $0x40,%r14
inc    %bpl
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r14)
movdqu %xmm0,0x30(%r14)
movdqu %xmm0,0x20(%r14)
movdqu %xmm0,0x10(%r14)
mov    %bpl,0x5(%r14)
movl   $0x0,0x73(%rsp)
movl   $0x0,0x70(%rsp)
mov    %r14,(%r12)
mov    0x70(%rsp),%eax
mov    0x73(%rsp),%ecx
mov    0x18(%rsp),%rsi
mov    %ecx,0x3(%rsi)
mov    %eax,(%rsi)
movb   $0x1,0xf(%r12)
movzbl %bpl,%r14d
cmp    (%rsp),%bpl
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x52fa>
cmpq   $0x0,0x68(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x637a>
mov    0x60(%rsp),%rax
mov    (%rax),%rcx
movzbl (%rsp),%edx
mov    %r12,%rdi
mov    %r14d,%esi
call   *0xd8b96(%rip)        # ADDR <_DYNAMIC+0x598>
mov    0x68(%rsp),%r13
test   %r13,%r13
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5352>
mov    %r14,0x20(%rsp)
mov    0x60(%rsp),%rax
mov    %rax,0x28(%rsp)
shl    $0x3,%r13
xor    %r14d,%r14d
movzbl (%rsp),%ebp
xchg   %ax,%ax
mov    0x28(%rsp),%rax
mov    (%rax,%r14,1),%rdx
mov    0x10(%rsp),%rdi
mov    %r12,%rsi
mov    %ebp,%ecx
mov    0x50(%rsp),%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
add    $0x8,%r14
cmp    %r14,%r13
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5320>
mov    0x68(%rsp),%rax
mov    0x20(%rsp),%r14
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5354>
xor    %eax,%eax
cmpb   $0x38,0x30(%rsp)
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x632f>
dec    %rax
mov    0x18(%rsp),%rsi
mov    (%rsi),%rcx
xor    %rcx,%rax
lea    -0x29283c(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rdx,%r14,8),%rax
xor    %rcx,%rax
mov    %rax,(%rsi)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x56eb>
cmp    $0x101,%r14d
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x54e0>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6133>
mov    %rax,%rbp
mov    0x10(%rsp),%r9
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
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5408>
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
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x53e2>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate::insert_with_path_flat+0x54bd>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5468>
add    $0x40,%rbp
mov    0x50(%rsp),%rbx
mov    %rbp,%rdi
xor    %esi,%esi
mov    %r14,%rdx
call   *0xd872c(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x18(%rsp),%r15
mov    0x8(%rsp),%r12
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5508>
mov    $0x10,%esi
mov    %r14,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rbp
test   %rax,%rax
mov    0x50(%rsp),%rbx
mov    0x18(%rsp),%r15
mov    0x8(%rsp),%r12
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x61a3>
mov    0x20(%rsp),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5529>
mov    %r14,%rdx
imul   0x30(%rsp),%rdx
lea    0x58(%rsp),%rsi
mov    %rbp,%rdi
call   *0xd85e7(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    (%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
mov    0x30(%rsp),%r9
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5b63>
lea    -0x29675c(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x8b45>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
mov    0x38(%rsp),%rsi
mov    0x90(%rsp),%r8
jmp    *%rcx
mov    %r13b,0x0(%rbp,%r14,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5bb8>
cmp    %r13b,0x1(%r10)
adc    $0x0,%rcx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f40>
cmp    %r13b,0x1(%r10)
adc    $0x0,%rcx
xor    %eax,%eax
cmp    %r13b,0x2(%r10)
setb   %al
cmp    %r13b,0x3(%r10)
adc    %rax,%rcx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2f40>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x618e>
mov    %rax,%r14
mov    0x10(%rsp),%r8
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
jg     ADDR <expanse_trie::mutate::insert_with_path_flat+0x55d3>
add    $0x40,%r14
pxor   %xmm0,%xmm0
movdqa %xmm0,0x70(%rax)
movdqa %xmm0,0x60(%rax)
movdqa %xmm0,0x50(%rax)
movdqa %xmm0,0x40(%rax)
mov    0x68(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x564e>
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
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5630>
movl   $0x0,0x73(%rsp)
movl   $0x0,0x70(%rsp)
mov    %r14,(%r12)
mov    0x70(%rsp),%eax
mov    0x73(%rsp),%ecx
mov    0x18(%rsp),%rdx
mov    %ecx,0x3(%rdx)
mov    %eax,(%rdx)
movb   $0xc,0xf(%r12)
movzbl 0x68(%rsp),%eax
dec    %al
mov    %al,0x8(%r12)
cmpq   $0x0,0x68(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6371>
mov    %r12,%rdi
mov    0x60(%rsp),%rax
mov    (%rax),%rcx
movzbl (%rsp),%edx
mov    $0x1,%esi
call   *0xd87e4(%rip)        # ADDR <_DYNAMIC+0x598>
cmpb   $0x1,(%rsp)
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x56eb>
shr    $0x8,%r13
mov    0x50(%rsp),%rcx
mov    %r13,0x40(%rcx)
mov    %r14,0x58(%rcx)
movq   $0x0,0x60(%rcx)
movzwl 0x68(%rsp),%eax
mov    %ax,0x70(%rcx)
mov    %r12,(%rcx)
movb   $0x1,0x48(%rcx)
movq   $0x1,0x50(%rcx)
movq   $0x0,0x68(%rcx)
mov    0x8(%rsp),%rdx
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60bd>
imul   %rbx,%r15
lea    0xc(%r15),%rax
and    $0xfffffffffffffff0,%rax
mov    0x10(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
cmp    $0x177,%r15
mov    0x50(%rsp),%r14
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x575b>
lea    -0x290ad9(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%r15,%rax,1),%edi
cmp    $0xff,%rdi
mov    0x40(%rsp),%rbx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5773>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6340>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x577c>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r15
mov    0x40(%rsp),%rbx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x60f4>
mov    %rdx,%rdi
call   *0xd83ac(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x38(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5815>
shl    $0x4,%rax
add    %rsp,%rax
add    $0xb8,%rax
lea    0xb8(%rsp),%rcx
lea    0xd59c2(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x57b9>
nopl   0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5815>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x57eb>
mov    0x8(%rdi),%r8
lea    -0x292c99(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x57b0>
mov    %rdi,(%r14,%r8,8)
mov    0x50(%r14),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6286>
mov    %sil,0x48(%r14,%rdi,1)
incq   0x50(%r14)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x57b0>
cmpq   $0x0,0x58(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5828>
mov    0x60(%rsp),%rdi
call   *0xd8300(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    $0x1,%al
and    $0x1,%al
add    $0x138,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
mov    %ecx,0x78(%rsp)
shr    $0x20,%rcx
mov    %cl,0x7c(%rsp)
mov    %eax,0x7d(%rsp)
shr    $0x20,%rax
mov    %al,0x81(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x58f4>
mov    %cx,0x78(%rsp)
shr    $0x10,%ecx
mov    %cl,0x7a(%rsp)
mov    %ax,0x7b(%rsp)
shr    $0x10,%eax
mov    %al,0x7d(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x58f4>
mov    %ecx,0x78(%rsp)
mov    %eax,0x7c(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x58f4>
mov    %rcx,0x78(%rsp)
xor    %eax,%eax
mov    0x40(%rsp),%r11
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x58f4>
mov    %cx,0x78(%rsp)
mov    %ax,0x7a(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x58f4>
mov    %ecx,0x78(%rsp)
shr    $0x20,%rcx
mov    %cx,0x7c(%rsp)
mov    %eax,0x7e(%rsp)
shr    $0x20,%rax
mov    %ax,0x82(%rsp)
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x58f4>
mov    %ecx,0x78(%rsp)
mov    %rcx,%rdx
shr    $0x20,%rdx
mov    %dx,0x7c(%rsp)
shr    $0x30,%rcx
mov    %cl,0x7e(%rsp)
mov    %eax,0x7f(%rsp)
mov    %rax,%rcx
shr    $0x20,%rcx
mov    %cx,0x83(%rsp)
shr    $0x30,%rax
mov    0x78(%rsp),%rcx
mov    %rcx,(%r8)
lea    0x80(%rsp),%rdx
mov    (%rdx),%ecx
mov    %ecx,0x8(%r8)
movzbl 0x4(%rdx),%ecx
mov    %cl,0xc(%r8)
mov    %al,0xd(%r8)
movb   $0x0,0xe(%r8)
lea    0x1(%rbp),%eax
mov    %al,0xf(%r8)
mov    $0x1,%al
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
shl    $0x4,%rdi
lea    (%rsp,%rdi,1),%rcx
add    $0xb8,%rcx
lea    0xb8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5967>
cs nopw 0x0(%rax,%rax,1)
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5999>
mov    0x8(%rdi),%r8
lea    -0x292e47(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x595a>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5950>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ea5>
lea    (%rdx,%rdx,4),%rax
mov    %r13d,0xa0(%rsp,%rax,1)
shr    $0x20,%r13
mov    %r13b,0xa4(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5a4c>
lea    (%rdx,%rdx,2),%rax
mov    %r13w,0xa0(%rsp,%rax,1)
shr    $0x10,%r13d
mov    %r13b,0xa2(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5a4c>
mov    %r13d,0xa0(%rsp,%rdx,4)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5a4c>
mov    %r13,0xa0(%rsp,%rdx,8)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5a4c>
mov    %r13w,0xa0(%rsp,%rdx,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5a4c>
lea    (%rdx,%rdx,2),%rax
mov    %r13d,0xa0(%rsp,%rax,2)
shr    $0x20,%r13
mov    %r13w,0xa4(%rsp,%rax,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5a4c>
lea    (%rsp,%rdx,8),%rax
add    $0xa0,%rax
sub    %rdx,%rax
mov    %r13d,(%rax)
mov    %r13,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%r13
mov    %r13b,0x6(%rax)
add    $0x2,%r15b
mov    %r15d,%eax
mul    %r10b
cmp    $0x10,%al
jae    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6143>
mov    0xa0(%rsp),%rax
mov    0x18(%rsp),%rdx
mov    %rax,(%rdx)
lea    0xa8(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x3(%r12)
mov    %eax,(%r12)
and    $0x70,%bpl
or     %dil,%bpl
mov    %bpl,0xf(%rdx)
test   %rsi,%rsi
mov    0x28(%rsp),%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
shl    $0x4,%rsi
lea    (%rsp,%rsi,1),%rcx
add    $0xb8,%rcx
lea    0xb8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5acd>
cs nopw 0x0(%rax,%rax,1)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5aff>
mov    0x8(%rdi),%r8
lea    -0x292fad(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5ac0>
mov    0x50(%rsp),%r9
mov    %rdi,(%r9,%r8,8)
mov    0x50(%r9),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ea5>
mov    %sil,0x48(%r9,%rdi,1)
incq   0x50(%r9)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5ac0>
lea    (%r14,%r14,4),%rax
mov    %r13d,0x0(%rbp,%rax,1)
shr    $0x20,%r13
mov    %r13b,0x4(%rbp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5bb8>
lea    (%r14,%r14,2),%rax
mov    %r13w,0x0(%rbp,%rax,1)
shr    $0x10,%r13d
mov    %r13b,0x2(%rbp,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5bb8>
mov    %r13d,0x0(%rbp,%r14,4)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5bb8>
mov    %r13,0x0(%rbp,%r14,8)
mov    0x38(%rsp),%rsi
mov    0x90(%rsp),%r8
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5bb8>
mov    %r13w,0x0(%rbp,%r14,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5bb8>
lea    (%r14,%r14,2),%rax
mov    %r13d,0x0(%rbp,%rax,2)
shr    $0x20,%r13
mov    %r13w,0x4(%rbp,%rax,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5bb8>
lea    0x0(,%r14,8),%rax
add    %rbp,%rax
sub    %r14,%rax
mov    %r13d,(%rax)
mov    %r13,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%r13
mov    %r13b,0x6(%rax)
mov    %r8,%rdx
sub    %r14,%rdx
jbe    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5bfc>
mov    %r14,%rax
imul   %r9,%rax
lea    (%rsp,%rax,1),%rsi
add    $0x58,%rsi
inc    %r14
imul   %r9,%r14
add    %rbp,%r14
imul   %r9,%rdx
mov    %r14,%rdi
mov    %r8,%r14
mov    %r9,%r12
call   *0xd7f24(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r12,%r9
mov    0x8(%rsp),%r12
mov    %r14,%r8
mov    0x38(%rsp),%rsi
lea    0x4(%r9),%eax
movl   $0x0,0x73(%rsp)
movl   $0x0,0x70(%rsp)
mov    %rbp,(%r15)
mov    0x70(%rsp),%ecx
mov    0x73(%rsp),%edx
mov    %edx,0x3(%r12)
mov    %ecx,(%r12)
mov    %al,0xf(%r15)
mov    0x8(%r15),%rax
xor    %rax,%r8
lea    -0x2930fe(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rcx,%r9,8),%r8
xor    %rax,%r8
mov    %r8,0x8(%r15)
test   %rsi,%rsi
mov    0x28(%rsp),%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
shl    $0x4,%rsi
lea    (%rsp,%rsi,1),%rcx
add    $0xb8,%rcx
lea    0xb8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c7d>
nopl   0x0(%rax,%rax,1)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5caf>
mov    0x8(%rdi),%r8
lea    -0x29315d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c70>
mov    %rdi,(%rbx,%r8,8)
mov    0x50(%rbx),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ea5>
mov    %sil,0x48(%rbx,%rdi,1)
incq   0x50(%rbx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5c70>
lea    0x0(,%rbp,2),%rax
add    %rbp,%rax
mov    0x28(%rsp),%rcx
mov    %ecx,0x0(%r13,%rax,2)
shr    $0x20,%rcx
mov    %cx,0x4(%r13,%rax,2)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5d72>
mov    0x28(%rsp),%rax
mov    %eax,0x0(%r13,%rbp,4)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5d72>
lea    0x0(,%rbp,4),%rax
add    %rbp,%rax
mov    0x28(%rsp),%rcx
mov    %ecx,0x0(%r13,%rax,1)
shr    $0x20,%rcx
mov    %cl,0x4(%r13,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5d72>
lea    0x0(,%rbp,2),%rax
add    %rbp,%rax
mov    0x28(%rsp),%rcx
mov    %cx,0x0(%r13,%rax,1)
shr    $0x10,%ecx
mov    %cl,0x2(%r13,%rax,1)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5d72>
lea    0x0(,%rbp,8),%rax
add    %r13,%rax
sub    %rbp,%rax
mov    0x28(%rsp),%rdx
mov    %edx,(%rax)
mov    %rdx,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%rdx
mov    %dl,0x6(%rax)
mov    0x40(%rsp),%r11
mov    0x18(%rsp),%rcx
mov    (%rcx),%rax
xor    %rax,%r8
and    %rsi,%r8
xor    %rax,%r8
mov    %r8,(%rcx)
mov    (%r11),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f8b>
mov    0x68(%rbx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f6c>
movq   $0x0,0x68(%rbx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f6c>
movzbl 0x49(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5de9>
mov    0x8(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x293297(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f6c>
movzbl 0x4a(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5e26>
mov    0x10(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2932d4(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f6c>
movzbl 0x4b(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5e63>
mov    0x18(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x293311(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f6c>
movzbl 0x4c(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5ea0>
mov    0x20(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29334e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f6c>
movzbl 0x4d(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5edd>
mov    0x28(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x29338b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f6c>
movzbl 0x4e(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f1a>
mov    0x30(%rbx),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2933c8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f6c>
movzbl 0x4f(%rbx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5f58>
mov    0x50(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x293406(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
mov    0x50(%rsp),%rbx
mov    0x40(%rsp),%r11
jne    ADDR <expanse_trie::mutate::insert_with_path_flat+0x62f8>
movq   $0xffffffffffffffff,0x40(%rbx)
movw   $0x0,0x70(%rbx)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
mov    $0x1,%al
test   %r14,%r14
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
shl    $0x4,%r14
lea    (%rsp,%r14,1),%rcx
add    $0xb8,%rcx
lea    0xb8(%rsp),%rdx
xor    %esi,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fcd>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x582a>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%r8d
cmp    $0x7,%r8
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x6003>
mov    0x8(%rdi),%rsi
lea    -0x2934ae(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fc0>
mov    %rdi,(%rbx,%rsi,8)
mov    0x50(%rbx),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate::insert_with_path_flat+0x2ea5>
mov    %r8b,0x48(%rbx,%rdi,1)
mov    0x50(%rbx),%rsi
inc    %rsi
mov    %rsi,0x50(%rbx)
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x5fc0>
lea    0xce930(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xb38>
mov    $0x7,%esi
call   *0xd7abd(%rip)        # ADDR <_DYNAMIC+0x208>
lea    -0x29343f(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc968>
lea    0xd5687(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x78a8>
mov    $0x13,%esi
call   *0xd7c6c(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x2b6b8b(%rip),%rdi        # ADDR <anon.ADDR.91.llvm.ADDR+0x260>
lea    0xd55de(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7818>
mov    $0x8,%esi
call   *0xd7c53(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x2b6b6c(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
lea    0xd56fd(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7950>
mov    $0x8,%esi
call   *0xd7c3a(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0xd562b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7890>
mov    %r14,%rdi
mov    %rbx,%rsi
call   *0xd7af7(%rip)        # ADDR <_DYNAMIC+0x2a0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638b>
lea    0xd5673(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x78f0>
mov    0x30(%rsp),%rdi
call   *0xd7ae0(%rip)        # ADDR <_DYNAMIC+0x2a0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638b>
lea    -0x2b6bb4(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
lea    0xd569d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7938>
mov    $0x8,%esi
call   *0xd7bf2(%rip)        # ADDR <_DYNAMIC+0x3d0>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638b>
lea    -0x2b6bd2(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
lea    0xd55bf(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7878>
mov    $0x8,%esi
call   *0xd7bd4(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x293656(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0xd4dce(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0xcdf57(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x9f(%rsp),%rdx
mov    $0x11,%esi
call   *0xd7aa4(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638b>
lea    0xce108(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
call   *0xd79cd(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0xd7a35(%rip)        # ADDR <_DYNAMIC+0x280>
lea    -0x29357c(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0xd55b7(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x78d8>
mov    $0x12,%esi
call   *0xd7b6c(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0xccf0d(%rip),%rcx        # ADDR <anon.ADDR.892.llvm.ADDR>
mov    %rdx,%rsi
mov    $0x7,%edx
mov    %rbx,%rdi
call   *0xd79d4(%rip)        # ADDR <_DYNAMIC+0x250>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638b>
lea    0xce0b0(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
mov    %rax,%rdi
call   *0xd7972(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0xd79da(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638b>
mov    $0x10,%edi
mov    %r14,%rsi
call   *0xd79c7(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0xce0d8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x460>
mov    $0x8,%esi
call   *0xd793d(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xd54c6(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7860>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xd7928(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xd5499(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7848>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xd7913(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xd53c4(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7788>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xd78fe(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x6,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x624f>
lea    0xd53d8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x77b8>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xd78e2(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xd53db(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x77d0>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0xd78cb(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xd5394(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x77a0>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xd78b6(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x8,%esi
call   *0xd78ab(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638b>
lea    0xd53b7(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x77e8>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xd7891(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xd53ba(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7800>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0xd787a(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x8,%esi
call   *0xd786f(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638b>
lea    0xd53c3(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7830>
mov    $0x8,%esi
mov    %r11,%rdi
call   *0xd7855(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xcddc6(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %rbp,%rdi
call   *0xd7840(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xcdd81(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
call   *0xd782e(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xcdd6f(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x624f>
lea    0xcdd8e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %rbx,%rdi
call   *0xd7808(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0xd4e81(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0xd77f1(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0xd7859(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x638b>
mov    $0x10,%edi
mov    %r13,%rsi
call   *0xd7849(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %r14,%rdi
lea    0xcdfcf(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4d8>
mov    $0x8,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6385>
lea    0xcdd01(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x628b>
lea    0xcdd20(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
call   *0xd79b2(%rip)        # ADDR <_DYNAMIC+0x418>
mov    $0x10,%edi
mov    0x90(%rsp),%rsi
call   *0xd7807(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0xd53c0(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7908>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x6381>
lea    0xd53cf(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7920>
xor    %edi,%edi
xor    %esi,%esi
call   *0xd7775(%rip)        # ADDR <_DYNAMIC+0x208>
ud2
mov    $0x10,%edi
mov    %r12,%rsi
call   *0xd77dd(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x639f>
jmp    ADDR <expanse_trie::mutate::insert_with_path_flat+0x639f>
mov    %rax,%rbx
cmpq   $0x0,0x58(%rsp)
je     ADDR <expanse_trie::mutate::insert_with_path_flat+0x63bb>
mov    0x90(%rsp),%rax
mov    (%rax),%rdi
call   *0xd776d(%rip)        # ADDR <free@GLIBC_2.2.5>
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
