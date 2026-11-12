push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x178,%rsp
mov    %r9d,%ebp
mov    %r8,0x48(%rsp)
mov    %rcx,%rbx
mov    %rsi,0x8(%rsp)
mov    %rdi,0x90(%rsp)
mov    0x1b0(%rsp),%rax
lea    0x50(%rax),%r11
xor    %r14d,%r14d
mov    %r11,0x38(%rsp)
mov    %rcx,0x98(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
movzbl %dl,%edx
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%r15
mov    %r15,%rsi
mov    %eax,%ecx
call   ADDR <expanse_trie::mutate::wrap_skip_level::<false>>
mov    %r15,%rdx
mov    %r13,%r11
data16 data16 cs nopw 0x0(%rax,%rax,1)
movzbl 0xf(%rdx),%r13d
cmp    $0xc,%r13d
mov    %rdx,0x18(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x11fb>
lea    -0x20bacf(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x1839>
movslq (%rcx,%r13,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    (%r11),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30a>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2de>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2de>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x10a>
mov    0x1b0(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200ad8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2de>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x157>
mov    0x1b0(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200b25(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2de>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1a4>
mov    0x1b0(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200b72(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2de>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f1>
mov    0x1b0(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200bbf(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2de>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x23e>
mov    0x1b0(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200c0c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2de>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x28b>
mov    0x1b0(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200c59(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2de>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d4>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200ca2(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
mov    0x18(%rsp),%rdx
cmp    $0x8,%bpl
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2e38>
mov    %r14,%r15
mov    0x8(%rsp),%rcx
addq   $0x40,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
mov    0x18(%rcx),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1589>
mov    (%r14),%rax
mov    %rax,0x18(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1645>
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a90>
mov    (%rdx),%rax
mov    %r14,%rcx
shl    $0x4,%rcx
mov    %rdx,0xf8(%rsp,%rcx,1)
mov    %bpl,0x100(%rsp,%rcx,1)
dec    %bpl
lea    0x0(,%rbp,8),%ecx
mov    %rbx,%rdx
shr    %cl,%rdx
movzbl %dl,%ecx
inc    %r14
shl    $0x4,%ecx
lea    (%rax,%rcx,1),%rdx
add    $0x40,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
cmp    $0x1,%bpl
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29c8>
lea    -0x1(%rbp),%eax
movzbl %al,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a0c>
lea    -0x200d9e(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rcx
shl    $0x8,%rcx
mov    0x18(%rsp),%rax
mov    0x8(%rax),%rax
mov    %rax,%rdx
xor    %rbx,%rdx
test   %rcx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29c8>
mov    (%r11),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x647>
mov    0x1b0(%rsp),%rdx
mov    0x68(%rdx),%rdx
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x620>
mov    0x1b0(%rsp),%rsi
movq   $0x0,0x68(%rsi)
cmp    $0x1,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x620>
movzbl 0x49(%rsi),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44c>
mov    0x1b0(%rsp),%rdi
mov    0x8(%rdi),%rdi
mov    0x8(%rdi),%r8
lea    -0x200e1a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x2,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x620>
mov    0x1b0(%rsp),%rsi
movzbl 0x4a(%rsi),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x499>
mov    0x1b0(%rsp),%rdi
mov    0x10(%rdi),%rdi
mov    0x8(%rdi),%r8
lea    -0x200e67(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x3,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x620>
mov    0x1b0(%rsp),%rsi
movzbl 0x4b(%rsi),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e6>
mov    0x1b0(%rsp),%rdi
mov    0x18(%rdi),%rdi
mov    0x8(%rdi),%r8
lea    -0x200eb4(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x4,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x620>
mov    0x1b0(%rsp),%rsi
movzbl 0x4c(%rsi),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x533>
mov    0x1b0(%rsp),%rdi
mov    0x20(%rdi),%rdi
mov    0x8(%rdi),%r8
lea    -0x200f01(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x5,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x620>
mov    0x1b0(%rsp),%rsi
movzbl 0x4d(%rsi),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x580>
mov    0x1b0(%rsp),%rdi
mov    0x28(%rdi),%rdi
mov    0x8(%rdi),%r8
lea    -0x200f4e(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x6,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x620>
mov    0x1b0(%rsp),%rsi
movzbl 0x4e(%rsi),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5cd>
mov    0x1b0(%rsp),%rdi
mov    0x30(%rdi),%rdi
mov    0x8(%rdi),%r8
lea    -0x200f9b(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x7,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x620>
mov    0x1b0(%rsp),%rsi
movzbl 0x4f(%rsi),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x616>
mov    0x1b0(%rsp),%rdi
mov    0x38(%rdi),%rdi
mov    0x8(%rdi),%r8
lea    -0x200fe4(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x8,%rcx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rcx
movq   $0xffffffffffffffff,0x40(%rcx)
movw   $0x0,0x70(%rcx)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
mov    0x18(%rsp),%rcx
add    $0x8,%rcx
movzbl %al,%r8d
inc    %r8
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0xa3(%rsp)
mov    %eax,0xa0(%rsp)
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
nopl   (%rax)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x68f3>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa0(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x680>
movzbl %dl,%edx
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%r15
mov    %r15,%rsi
mov    %eax,%ecx
mov    %r11,%r13
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1579>
mov    (%rdx),%r13
movzbl 0x74(%r13),%r10d
movzbl %r10b,%edi
mov    %ebp,%eax
sub    %dil,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1089>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a1e>
lea    -0x2010cf(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x18(%rsp),%rdx
mov    0x8(%rdx),%r8
mov    %r8,%rcx
xor    %rbx,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1089>
cmp    $0x7,%r10b
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a7e>
mov    %rdx,%rcx
lea    -0x201109(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    (%r11),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x99b>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x96f>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x96f>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x79b>
mov    0x1b0(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201169(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x96f>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x7e8>
mov    0x1b0(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2011b6(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x96f>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x835>
mov    0x1b0(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201203(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x96f>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x882>
mov    0x1b0(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201250(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x96f>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x8cf>
mov    0x1b0(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20129d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x96f>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x91c>
mov    0x1b0(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2012ea(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x96f>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x965>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201333(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
mov    0x18(%rsp),%rcx
mov    %r11,%r13
lea    0x8(%rcx),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0xa3(%rsp)
mov    %ecx,0xa0(%rsp)
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
nopl   0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x68f3>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa0(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x9d0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47>
mov    (%rdx),%rdx
movzbl 0x5(%rdx),%esi
mov    %ebp,%eax
sub    %sil,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x113b>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a1e>
movzbl %sil,%edi
lea    -0x201409(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x18(%rsp),%r9
mov    0x8(%r9),%r8
mov    %r8,%rcx
xor    %rbx,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x113b>
cmp    $0x7,%sil
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a7e>
mov    %r9,%rcx
lea    -0x201443(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    (%r11),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xcd5>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xca9>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xca9>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xad5>
mov    0x1b0(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2014a3(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xca9>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xb22>
mov    0x1b0(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2014f0(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xca9>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xb6f>
mov    0x1b0(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20153d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xca9>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xbbc>
mov    0x1b0(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20158a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xca9>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc09>
mov    0x1b0(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2015d7(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xca9>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc56>
mov    0x1b0(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201624(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xca9>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc9f>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20166d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
mov    0x18(%rsp),%rcx
mov    %r11,%r13
lea    0x8(%rcx),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0xa3(%rsp)
mov    %ecx,0xa0(%rsp)
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x68f3>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa0(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xd10>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47>
mov    (%rdx),%r15
mov    %rbp,%rax
movzbl 0x5(%r15),%ebp
mov    %rax,%r9
sub    %bpl,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x11a3>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a1e>
movzbl %bpl,%edi
lea    -0x20174e(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x18(%rsp),%rdx
mov    0x8(%rdx),%r8
mov    %r8,%rcx
xor    %rbx,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x11a3>
cmp    $0x7,%bpl
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a7e>
mov    %rdx,%rcx
lea    -0x201788(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    0x38(%rsp),%r10
mov    (%r10),%rax
test   %rax,%rax
mov    %r9,%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1022>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xff6>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xff6>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xe22>
mov    0x1b0(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2017f0(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xff6>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xe6f>
mov    0x1b0(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20183d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xff6>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xebc>
mov    0x1b0(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20188a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xff6>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf09>
mov    0x1b0(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2018d7(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xff6>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf56>
mov    0x1b0(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201924(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xff6>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xfa3>
mov    0x1b0(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201971(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xff6>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xfec>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2019ba(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r10)
movq   $0x0,0x10(%r10)
mov    0x18(%rsp),%rcx
mov    %r10,%r13
lea    0x8(%rcx),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0xa3(%rsp)
mov    %ecx,0xa0(%rsp)
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x68f3>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa0(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1060>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47>
mov    %r14,0x60(%rsp)
lea    -0x1(%r10),%esi
lea    0x0(,%rsi,8),%ecx
mov    %rbx,%r14
shr    %cl,%r14
mov    %r14d,%r15d
shr    $0x5,%r15d
and    $0x7,%r15d
mov    0x0(%r13,%r15,4),%eax
mov    $0x1,%edx
mov    %r14d,%ecx
shl    %cl,%edx
bt     %r14d,%eax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16b0>
mov    0x60(%rsp),%r14
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6aa5>
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
mov    %r14,%rcx
shl    $0x4,%rcx
shr    $0x14,%eax
and    $0xfffffff0,%eax
add    0x20(%r13,%r15,8),%rax
mov    0x18(%rsp),%rdx
mov    %rdx,0xf8(%rsp,%rcx,1)
mov    %r10b,0x100(%rsp,%rcx,1)
inc    %r14
mov    %esi,%ebp
mov    %rax,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
lea    -0x1(%rsi),%edi
lea    0x0(,%rdi,8),%ecx
mov    %rbx,%rax
shr    %cl,%rax
movzbl 0x4(%rdx),%ecx
test   %ecx,%ecx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27c7>
movzbl 0x8(%rdx),%r8d
cmp    %al,%r8b
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1a24>
xor    %r9d,%r9d
cmp    $0x7,%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b10>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x18(%rsp),%rcx
mov    %rcx,0xf8(%rsp,%rax,1)
mov    %sil,0x100(%rsp,%rax,1)
inc    %r14
shl    $0x4,%r9d
add    %r9,%rdx
add    $0x10,%rdx
mov    %edi,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
lea    -0x1(%rbp),%edi
lea    0x0(,%rdi,8),%ecx
mov    %rbx,%rdx
shr    %cl,%rdx
mov    %edx,%ecx
and    $0xf,%cl
mov    $0x1,%r12d
shl    %cl,%r12d
mov    %edx,%eax
and    $0xf,%eax
movzwl 0x6(%r15),%ecx
bt     %eax,%ecx
movzbl 0x4(%r15),%eax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1aa9>
cmp    $0x4,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1a66>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x20ad>
cmp    %dl,0x8(%r15)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x20ad>
xor    %ecx,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x20c4>
cmp    $0x7f,%r13d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x68da>
lea    -0x5(%r13),%eax
cmp    $0x7,%al
mov    %r14,0x60(%rsp)
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2a5d>
mov    %rbp,0x58(%rsp)
mov    (%r11),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1491>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1465>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1465>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1291>
mov    0x1b0(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201c5f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1465>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12de>
mov    0x1b0(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201cac(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1465>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x132b>
mov    0x1b0(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201cf9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1465>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1378>
mov    0x1b0(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201d46(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1465>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x13c5>
mov    0x1b0(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201d93(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1465>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1412>
mov    0x1b0(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201de0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1465>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x145b>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201e29(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
mov    0x18(%rsp),%rdx
lea    -0x4(%r13),%eax
lea    0x8(%rdx),%rcx
mov    %rcx,0x50(%rsp)
movzbl %al,%r14d
mov    0x8(%rdx),%rax
lea    -0x201e95(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%r14,8),%r11
mov    %r11,%rbp
and    %rax,%rbp
lea    0x1(%rbp),%r8
mov    0x58(%rsp),%rcx
cmp    %r14b,%cl
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f50>
sub    %r14b,%cl
movzbl %cl,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a0c>
lea    -0x201ec8(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rdi,8),%rdx
lea    0x0(,%r14,8),%r15d
mov    %r15d,%ecx
shl    %cl,%rdx
xor    %rbx,%rax
test   %rax,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f58>
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0xa3(%rsp)
mov    %eax,0xa0(%rsp)
mov    0x58(%rsp),%rbp
movzbl %bpl,%eax
lea    0x1(%rbp),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
mov    0x38(%rsp),%r9
mov    0x60(%rsp),%r14
nopl   0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x68f3>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa0(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1540>
movzbl %dl,%edx
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%r15
mov    %r15,%rsi
mov    %eax,%ecx
mov    %r9,%r13
call   ADDR <expanse_trie::mutate::wrap_skip_level::<false>>
mov    %r13,%r11
mov    %r15,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
mov    %r11,%r13
movq   $0x0,0xa0(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa0(%rsp),%rdi
call   *0x170f50(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69c6>
mov    0xa0(%rsp),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69c6>
mov    $0x1000,%edx
mov    %r14,%rdi
xor    %esi,%esi
call   *0x170f37(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%r14)
movq   $0x1,0x8(%r14)
mov    %r14,0x220(%rdi)
lea    0xfc0(%r14),%rax
mov    $0x41,%ecx
data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
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
jg     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1610>
add    $0x40,%r14
mov    0x18(%rsp),%rdx
mov    %r13,%r11
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r14)
movdqu %xmm0,0x30(%r14)
movdqu %xmm0,0x20(%r14)
movdqu %xmm0,0x10(%r14)
movb   $0x8,0x5(%r14)
movl   $0x0,0xa3(%rsp)
movl   $0x0,0xa0(%rsp)
mov    %r14,(%rdx)
mov    0xa0(%rsp),%eax
mov    0xa3(%rsp),%ecx
mov    0x18(%rsp),%rdx
mov    %eax,0x8(%rdx)
mov    0x18(%rsp),%rax
mov    %ecx,0xb(%rax)
mov    0x18(%rsp),%rdx
movb   $0x1,0xf(%rdx)
mov    $0x8,%bpl
mov    %r15,%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
movdqa 0x0(%r13),%xmm0
movdqa 0x10(%r13),%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x1,%xmm2
movdqa -0x226a0d(%rip),%xmm3        # ADDR <anon.ADDR.407.llvm.ADDR+0x10>
pand   %xmm3,%xmm2
psubb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
movdqa -0x226771(%rip),%xmm4        # ADDR <anon.ADDR.407.llvm.ADDR+0x2c0>
pand   %xmm4,%xmm2
psrlw  $0x2,%xmm1
pand   %xmm4,%xmm1
paddb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x4,%xmm2
paddb  %xmm1,%xmm2
movdqa -0x226ae7(%rip),%xmm5        # ADDR <anon.ADDR.283.llvm.ADDR+0xf6>
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
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d45>
cmp    %r10b,%bpl
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2148>
cmp    $0x8,%r10b
mov    0x60(%rsp),%r14
mov    0x18(%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a7e>
lea    -0x202173(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%r8
and    0x8(%rcx),%r8
mov    (%r11),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1a04>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19dd>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19dd>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1809>
mov    0x1b0(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2021d7(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19dd>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1856>
mov    0x1b0(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202224(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19dd>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x18a3>
mov    0x1b0(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202271(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19dd>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x18f0>
mov    0x1b0(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2022be(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19dd>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x193d>
mov    0x1b0(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20230b(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19dd>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x198a>
mov    0x1b0(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202358(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19dd>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19d3>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2023a1(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
inc    %r8
inc    %r10b
movzbl %bpl,%ecx
movzbl %r10b,%edx
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%r15
mov    %r15,%rsi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b6>
cmp    $0x1,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1df2>
cmp    %al,%r8b
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x210d>
lea    0x10(%rdx),%r9
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27b9>
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27c7>
movzbl 0x9(%rdx),%r10d
mov    %r10b,0xa(%rdx)
mov    %r8b,0x9(%rdx)
movups 0x20(%rdx),%xmm0
movups %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27bd>
cmp    $0x8,%al
mov    $0x8,%ecx
cmovb  %eax,%ecx
movzbl %dl,%esi
movd   %esi,%xmm0
punpcklbw %xmm0,%xmm0
pshuflw $0x0,%xmm0,%xmm0
pshufd $0x44,%xmm0,%xmm0
movq   0x8(%r15),%xmm1
mov    $0xffffffff,%esi
shl    %cl,%esi
pcmpeqb %xmm0,%xmm1
pmovmskb %xmm1,%ecx
not    %esi
and    %ecx,%esi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1aa9>
tzcnt  %esi,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x20c4>
cmp    $0x7,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2413>
mov    0x38(%rsp),%r10
mov    (%r10),%rax
test   %rax,%rax
mov    %r9,%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d2b>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d04>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d04>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1b30>
mov    0x1b0(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2024fe(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d04>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1b7d>
mov    0x1b0(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20254b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d04>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1bca>
mov    0x1b0(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202598(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d04>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c17>
mov    0x1b0(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2025e5(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d04>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c64>
mov    0x1b0(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202632(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d04>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cb1>
mov    0x1b0(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20267f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d04>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cfa>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2026c8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r10)
movq   $0x0,0x10(%r10)
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%r15
mov    %r15,%rsi
mov    %r10,%r13
call   ADDR <expanse_trie::mutate::upgrade_l7_to_b::<false>>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x157e>
movzwl 0x60(%r13,%r15,2),%r8d
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
mov    %rax,0x58(%rsp)
test   %r8,%r8
mov    %r10,0x40(%rsp)
mov    %rsi,0x50(%rsp)
mov    %r13,0x10(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x248b>
mov    $0x20,%ebp
cmp    $0x1,%r8d
mov    0x8(%rsp),%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2495>
cmp    $0x10,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x282c>
cmp    $0x3,%r8w
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1dde>
lea    0x4(%r8),%eax
lea    0x3(%r8),%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
mov    0x8(%rsp),%rcx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2961>
mov    %r8d,%ebp
shl    $0x4,%ebp
add    $0x40,%ebp
and    $0x1c0,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2495>
mov    $0x1,%r9d
cmp    %al,0x9(%rdx)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1168>
cmp    $0x2,%cl
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1a2d>
mov    $0x2,%r9d
cmp    %al,0xa(%rdx)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1168>
cmp    $0x3,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1a2d>
mov    (%r11),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2093>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x206c>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x206c>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1e98>
mov    0x1b0(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202866(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x206c>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1ee5>
mov    0x1b0(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2028b3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x206c>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f32>
mov    0x1b0(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202900(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x206c>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f7f>
mov    0x1b0(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20294d(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x206c>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1fcc>
mov    0x1b0(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20299a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x206c>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2019>
mov    0x1b0(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2029e7(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x206c>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2062>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202a30(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%r15
mov    %r15,%rsi
mov    %r11,%r13
call   ADDR <expanse_trie::mutate::upgrade_l3_to_l7::<false>>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x157e>
cmp    $0x2,%al
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x23d8>
mov    $0x1,%ecx
cmp    %dl,0x9(%r15)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x23d8>
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6ae4>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x18(%rsp),%rdx
mov    %rdx,0xf8(%rsp,%rax,1)
mov    %bpl,0x100(%rsp,%rax,1)
cmp    $0x7,%ecx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6af9>
inc    %r14
shl    $0x4,%ecx
lea    (%r15,%rcx,1),%rdx
add    $0x10,%rdx
mov    %edi,%ebp
mov    0x38(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
movzbl 0x9(%rdx),%r9d
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2126>
mov    $0x2,%r8d
cmp    %al,%r9b
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27ca>
mov    $0x1,%r8d
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27ca>
mov    %r9b,0xa(%rdx)
movdqu 0x20(%rdx),%xmm0
movdqu %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27ca>
mov    (%r11),%rax
test   %rax,%rax
mov    0x60(%rsp),%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x23be>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x21c3>
mov    0x1b0(%rsp),%rsi
mov    0x8(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202b91(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2210>
mov    0x1b0(%rsp),%rsi
mov    0x10(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202bde(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x225d>
mov    0x1b0(%rsp),%rsi
mov    0x18(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202c2b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x22aa>
mov    0x1b0(%rsp),%rsi
mov    0x20(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202c78(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x22f7>
mov    0x1b0(%rsp),%rsi
mov    0x28(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202cc5(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2344>
mov    0x1b0(%rsp),%rsi
mov    0x30(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202d12(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x238d>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x202d5b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%r15
mov    %r15,%rsi
mov    %r11,%r13
call   ADDR <expanse_trie::mutate::upgrade_b_to_u::<false>>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x157e>
cmp    $0x3,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2413>
cmp    %dl,0xa(%r15)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2413>
cmp    $0x7,%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6ae4>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x18(%rsp),%rcx
mov    %rcx,0xf8(%rsp,%rax,1)
mov    %bpl,0x100(%rsp,%rax,1)
mov    $0x2,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x20f3>
mov    %rdi,%rbx
lea    0x8(%r15),%rdi
lea    0x10(%r15),%r13
movzbl %dl,%r8d
mov    $0x7,%edx
mov    %r13,%rsi
mov    %rax,%rcx
call   *0x170375(%rip)        # ADDR <_DYNAMIC+0x5a0>
incb   0x4(%r15)
or     %r12w,0x6(%r15)
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b25>
mov    %r14,%rcx
shl    $0x4,%rcx
mov    0x18(%rsp),%rdx
mov    %rdx,0xf8(%rsp,%rcx,1)
mov    %bpl,0x100(%rsp,%rcx,1)
cmp    $0x7,%eax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b3a>
inc    %r14
shl    $0x4,%eax
add    %rax,%r13
mov    %ebx,%ebp
mov    %r13,%rdx
mov    0x38(%rsp),%r11
mov    0x98(%rsp),%rbx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
mov    $0x10,%ebp
mov    0x8(%rsp),%rcx
lea    -0x200d63(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl 0x0(%rbp,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24e4>
add    %rbp,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3e,%r13b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b76>
mov    0x10(%rcx,%r13,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2523>
mov    %r8,%r12
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%r13,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x26b8>
mov    0x10(%rsp),%r13
mov    %r8,%r12
lock add %rbp,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
mov    $0x1,%esi
mov    %rbp,%rdi
call   *0x16fff6(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r8
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x26ce>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c5e>
cmp    $0x100,%ebp
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2919>
mov    %rbp,0x30(%rsp)
mov    %r8,0x20(%rsp)
movq   $0x0,0xa0(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa0(%rsp),%rdi
call   *0x16ffa3(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69c6>
mov    0xa0(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69c6>
mov    $0x1000,%edx
xor    %esi,%esi
mov    %rdi,0x28(%rsp)
call   *0x16ff88(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x28(%rsp),%r9
mov    0x8(%rsp),%rbp
mov    0x220(%rbp),%rax
mov    %rax,(%r9)
mov    %r13,0x8(%r9)
mov    %r9,0x220(%rbp)
mov    $0xfc0,%ax
xor    %edx,%edx
mov    0x30(%rsp),%r10
div    %r10w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x25f6>
mov    %rcx,%rax
nopl   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rsi
imul   %r10,%rsi
lea    (%r9,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%rbp,%r13,8),%r8
mov    %r8,0x40(%r9,%rsi,1)
mov    %rdi,0x10(%rbp,%r13,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x25d0>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x26a5>
lea    -0x4(%rax),%rcx
mov    0x30(%rsp),%r9
imul   %r9,%rcx
add    $0x40,%rcx
lea    0x0(,%r9,4),%edx
neg    %rdx
lea    -0x3(%rax),%rsi
imul   %r9,%rsi
add    $0x40,%rsi
lea    -0x2(%rax),%rdi
imul   %r9,%rdi
add    $0x40,%rdi
lea    -0x1(%rax),%r8
imul   %r9,%r8
add    $0x40,%r8
mov    0x28(%rsp),%r9
nopl   0x0(%rax,%rax,1)
lea    (%r9,%r8,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x10(%rbp,%r13,8)
lea    (%r9,%rdi,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%rbp,%r13,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%rbp,%r13,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%rbp,%r13,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2650>
mov    0x28(%rsp),%rdi
add    $0x40,%rdi
mov    0x20(%rsp),%r12
mov    0x30(%rsp),%rbp
xor    %esi,%esi
mov    %rbp,%rdx
mov    %rdi,%r13
call   *0x16fe4a(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r13,%r8
mov    0x10(%rsp),%r13
mov    %r12,%rbp
test   %r12,%r12
mov    0x38(%rsp),%r11
mov    0x50(%rsp),%rsi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x289c>
mov    0x20(%r13,%r15,8),%rsi
mov    %rsi,0x20(%rsp)
mov    %rbp,%r13
mov    0x58(%rsp),%rbp
shl    $0x4,%ebp
mov    %r8,%rdi
mov    %rbp,%rdx
mov    %r8,%r12
call   *0x16fd18(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r12,0x28(%rsp)
lea    (%r12,%rbp,1),%rdi
add    $0x10,%rdi
mov    0x20(%rsp),%r12
add    %r12,%rbp
mov    %r13,%rdx
sub    0x58(%rsp),%rdx
shl    $0x4,%rdx
mov    %rbp,%rsi
call   *0x16fcee(%rip)        # ADDR <memcpy@GLIBC_2.14>
test   %r12,%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x691e>
mov    %r12,%r9
mov    %r13,%rax
cmp    $0x3,%r13w
mov    0x38(%rsp),%r11
mov    0x8(%rsp),%rcx
mov    0x50(%rsp),%rsi
mov    %r13,%rbp
mov    0x28(%rsp),%r8
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x276f>
cmp    $0x11,%bp
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x284b>
lea    0x3(%rbp),%eax
and    $0x3c,%eax
shl    $0x4,%eax
lea    -0x201040(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rax,%rdx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x286f>
sub    %rax,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b8b>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%r9)
mov    %r9,0x10(%rcx,%rdi,8)
mov    0x10(%rsp),%r13
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x289c>
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
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6acf>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x18(%rsp),%rcx
mov    %rcx,0xf8(%rsp,%rax,1)
add    %r8,%rdx
add    $0x10,%rdx
mov    %sil,0x100(%rsp,%rax,1)
inc    %r14
mov    %edi,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
cmp    $0x18,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2926>
mov    $0x180,%ebp
cmp    $0x10,%r8d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24e9>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2961>
mov    %rbp,%rdx
mov    $0x180,%eax
cmp    $0x19,%dx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x286f>
mov    $0x200,%eax
cmp    $0x21,%bp
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x286f>
mov    %ebp,%eax
shl    $0x4,%eax
add    $0x30,%eax
and    $0xffffffc0,%eax
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
mov    %r9,%rdi
call   *0x16fbb0(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x38(%rsp),%r11
mov    0x10(%rsp),%r13
mov    0x50(%rsp),%rsi
mov    0x28(%rsp),%r8
mov    0x58(%rsp),%rax
shl    $0x4,%eax
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r8,%rax,1)
mov    %r8,0x20(%r13,%r15,8)
mov    $0x1,%eax
mov    %r14d,%ecx
shl    %cl,%rax
inc    %ebp
mov    %bp,0x60(%r13,%r15,2)
shr    $0x3,%r14d
and    $0x18,%r14d
or     %rax,0x0(%r13,%r14,1)
mov    0x60(%rsp),%r14
cmp    $0x7,%r14
mov    0x40(%rsp),%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b61>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x18(%rsp),%rcx
mov    %rcx,0xf8(%rsp,%rax,1)
mov    %dl,0x100(%rsp,%rax,1)
inc    %r14
mov    0x58(%rsp),%rdx
shl    $0x4,%edx
add    0x20(%r13,%r15,8),%rdx
mov    %esi,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x70>
mov    %r8,%r12
mov    0x10(%rsp),%r13
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2504>
cmp    $0x20,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x293e>
mov    $0x200,%ebp
cmp    $0x18,%r8d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24e9>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2961>
mov    $0x240,%ebp
cmp    $0x20,%r8d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24e9>
lea    0x4(%r8),%eax
lea    0x3(%r8),%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
mov    0x8(%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29b7>
mov    0x20(%r13,%r15,8),%r12
mov    0x58(%rsp),%rax
mov    %eax,%r13d
shl    $0x4,%r13d
lea    (%r12,%r13,1),%rsi
lea    (%r12,%r13,1),%rdi
add    $0x10,%rdi
mov    %r8,%rdx
sub    %rax,%rdx
shl    $0x4,%rdx
mov    %r8,0x20(%rsp)
mov    %r11,%rbp
call   *0x16fb5a(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x50(%rsp),%rsi
mov    %rbp,%r11
mov    0x20(%rsp),%rbp
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12,%r13,1)
mov    0x10(%rsp),%r13
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x28b3>
mov    %r8d,%ebp
shl    $0x4,%ebp
add    $0x40,%ebp
and    $0xffffffc0,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24e9>
mov    %rbp,0x58(%rsp)
mov    %ebx,%ebp
shr    $0x5,%ebp
and    $0x7,%ebp
mov    $0x1,%eax
mov    %ebx,%ecx
shl    %cl,%eax
mov    0x18(%rsp),%rcx
mov    (%rcx),%rsi
mov    (%rsi,%rbp,4),%r13d
dec    %eax
and    %r13d,%eax
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
imul   $0x1010101,%ecx,%r8d
shr    $0x18,%r8d
bt     %ebx,%r13d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ff9>
mov    0x20(%rsi,%rbp,8),%rax
lea    (%rax,%r8,8),%rcx
mov    (%rax,%r8,8),%rdx
mov    0x48(%rsp),%rsi
mov    %rsi,(%rax,%r8,8)
mov    0x90(%rsp),%rax
movq   $0x1,(%rax)
mov    %rdx,0x8(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f47>
test   %r13b,%r13b
js     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6905>
mov    %r13d,%r14d
shr    $0x4,%r14d
test   %r14b,%r14b
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6905>
mov    %r13d,%r10d
and    $0xf,%r10d
lea    0x1(%r10),%rsi
mov    %rsi,%r15
imul   %r14,%r15
cmp    $0xf,%r15
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6905>
mov    (%r11),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d03>
mov    0x1b0(%rsp),%rcx
mov    0x68(%rcx),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cdc>
mov    0x1b0(%rsp),%rdx
movq   $0x0,0x68(%rdx)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cdc>
movzbl 0x49(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2b08>
mov    0x1b0(%rsp),%rdi
mov    0x8(%rdi),%r9
mov    0x8(%r9),%rdi
lea    -0x2034d6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cdc>
mov    0x1b0(%rsp),%rdx
movzbl 0x4a(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2b55>
mov    0x1b0(%rsp),%rdi
mov    0x10(%rdi),%r9
mov    0x8(%r9),%rdi
lea    -0x203523(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cdc>
mov    0x1b0(%rsp),%rdx
movzbl 0x4b(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ba2>
mov    0x1b0(%rsp),%rdi
mov    0x18(%rdi),%r9
mov    0x8(%r9),%rdi
lea    -0x203570(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cdc>
mov    0x1b0(%rsp),%rdx
movzbl 0x4c(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2bef>
mov    0x1b0(%rsp),%rdi
mov    0x20(%rdi),%r9
mov    0x8(%r9),%rdi
lea    -0x2035bd(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cdc>
mov    0x1b0(%rsp),%rdx
movzbl 0x4d(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c3c>
mov    0x1b0(%rsp),%rdi
mov    0x28(%rdi),%r9
mov    0x8(%r9),%rdi
lea    -0x20360a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cdc>
mov    0x1b0(%rsp),%rdx
movzbl 0x4e(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c89>
mov    0x1b0(%rsp),%rdi
mov    0x30(%rdi),%r9
mov    0x8(%r9),%rdi
lea    -0x203657(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cdc>
mov    0x1b0(%rsp),%rdx
movzbl 0x4f(%rdx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cd2>
mov    0x1b0(%rsp),%rdi
mov    0x38(%rdi),%r9
mov    0x8(%r9),%rdi
lea    -0x2036a0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
mov    0x1b0(%rsp),%rax
movq   $0xffffffffffffffff,0x40(%rax)
movw   $0x0,0x70(%rax)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r11)
movq   $0x0,0x10(%r11)
lea    0x0(,%r14,8),%ecx
mov    $0xffffffffffffffff,%rbp
shl    %cl,%rbp
not    %rbp
and    %rbp,%rbx
mov    0x18(%rsp),%rcx
lea    0x8(%rcx),%rdi
test   %r10b,%r10b
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30cb>
mov    %r10,0x58(%rsp)
movzbl %r14b,%edx
mov    %rdi,0x98(%rsp)
mov    %rdx,0x10(%rsp)
mov    %rbx,%rcx
mov    %rsi,%rbp
call   ADDR <expanse_trie::leaf::locate>
mov    %rdx,%r11
mov    0x18(%rsp),%rcx
mov    (%rcx),%r8
test   $0x1,%al
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3102>
mov    $0x7,%al
movzbl %al,%eax
div    %r14b
lea    0x4(%rbp),%rcx
cmp    %bpl,%al
mov    %rbp,0x28(%rsp)
mov    %r11,0x50(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x32af>
mov    %rbp,%rdx
lea    0x1(%rbp),%r9
cmpb   $0x1,0x58(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3458>
lea    0x3(%rdx),%rax
xor    %rax,%rcx
cmp    $0x4,%ecx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3458>
lea    (%r8,%r11,8),%rsi
cmp    %r11,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x412f>
lea    0x1(%r11),%rbp
mov    %r9,0x30(%rsp)
lea    (%r8,%r11,8),%rdi
add    $0x8,%rdi
sub    %r11d,%edx
shl    $0x3,%edx
mov    %r8,%r12
mov    %rsi,0x8(%rsp)
call   *0x16f71c(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x48(%rsp),%rax
mov    0x50(%rsp),%rcx
mov    %rax,(%r12,%rcx,8)
mov    0x98(%rsp),%r12
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %eax,0x78(%rsp)
mov    %ecx,0x7b(%rsp)
mov    0x50(%rsp),%rdx
imul   %r14,%rdx
imul   %r14,%rbp
lea    0x1662d1(%rip),%r9        # ADDR <anon.ADDR.2.llvm.ADDR+0x9a0>
lea    0x78(%rsp),%rdi
mov    $0x7,%esi
mov    %r15,%rcx
mov    %rbp,%r8
call   ADDR <<[u8]>::copy_within::<core::ops::range::Range<usize>>>
mov    0x8(%rsp),%rsi
mov    0x30(%rsp),%r9
mov    0x50(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4150>
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2e51>
lea    0x0(,%rbp,8),%ecx
mov    $0xffffffffffffffff,%rax
shl    %cl,%rax
not    %rax
and    %rax,%rbx
mov    %ebx,%r8d
shr    $0x10,%r8d
mov    %ebx,%ecx
shr    $0x18,%ecx
mov    %rbx,%rdx
shr    $0x20,%rdx
mov    %rbx,%rsi
shr    $0x28,%rsi
mov    %rbx,%rdi
shr    $0x30,%rdi
shl    $0x4,%bpl
mov    0x18(%rsp),%rax
mov    0x48(%rsp),%r9
mov    %r9,(%rax)
mov    %bl,0x8(%rax)
mov    %bh,0x9(%rax)
mov    %r8b,0xa(%rax)
mov    %cl,0xb(%rax)
mov    %dl,0xc(%rax)
mov    %sil,0xd(%rax)
mov    %dil,0xe(%rax)
mov    %bpl,0xf(%rax)
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f33>
shl    $0x4,%r14
lea    (%rsp,%r14,1),%rax
add    $0xf8,%rax
lea    0xf8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ed9>
mov    0x1b0(%rsp),%rsi
mov    %dl,0x48(%rsi,%rdi,1)
incq   0x50(%rsi)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f33>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f0b>
mov    0x8(%rsi),%rdi
lea    -0x2038d9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%r8
mov    %r8,%r9
and    %rdi,%r9
inc    %r9
xor    %rdi,%r9
and    %r8,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    (%r11),%rdi
lea    -0x1(%rdi),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ed0>
mov    0x1b0(%rsp),%r8
mov    %rsi,(%r8,%rdi,8)
mov    0x50(%r8),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ec0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x540a>
mov    0x90(%rsp),%rax
movq   $0x0,(%rax)
mov    0x18(%rsp),%rcx
mov    %rcx,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57fa>
lea    0x0(,%r14,8),%r15d
mov    $0xffffffffffffffff,%rdx
mov    %r15d,%ecx
shl    %cl,%rdx
not    %rdx
and    %rbx,%rdx
mov    0x18(%rsp),%rax
mov    (%rax),%r12
cmp    $0x3,%r8
mov    %r12,0x10(%rsp)
mov    %rdx,0x28(%rsp)
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2fdf>
mov    %r8,%rax
test   %r8,%r8
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3095>
cmp    $0x1,%r14
sete   %r10b
lea    0x8(%r14),%r9
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x8b(%rsp)
mov    %eax,0x88(%rsp)
mov    $0x1,%eax
mov    %rax,0xe0(%rsp)
xor    %r13d,%r13d
movq   $0x0,0x70(%rsp)
movq   $0x0,0x40(%rsp)
mov    0x8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44dd>
cmp    $0x11,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x307b>
mov    %rbp,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3095>
mov    %r13d,%eax
shr    $1,%eax
and    $0x55555555,%eax
mov    %r13d,%ecx
sub    %eax,%ecx
mov    %ecx,%eax
and    $0x33333333,%eax
shr    $0x2,%ecx
and    $0x33333333,%ecx
add    %eax,%ecx
mov    %ecx,%eax
shr    $0x4,%eax
add    %ecx,%eax
and    $0xf0f0f0f,%eax
imul   $0x1010101,%eax,%r9d
shr    $0x18,%r9d
test   %r13d,%r13d
mov    %r14,0x60(%rsp)
mov    %r8,0x50(%rsp)
mov    %rsi,0x10(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3534>
lea    -0x1(%r13),%eax
test   %eax,%r13d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x32a2>
cmp    $0x10,%r9d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3330>
lea    0x4(%r9),%eax
and    $0x1c,%eax
cmp    $0x2,%r9
mov    0x8(%rsp),%rcx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3349>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3589>
mov    $0x18,%eax
cmp    $0x19,%r8
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3095>
mov    $0x20,%eax
cmp    $0x21,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2fe9>
lea    (%r12,%rax,8),%rdi
lea    -0x1(%r14),%eax
cmp    $0x5,%eax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x318e>
lea    -0x20ea4d(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x18dd>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
movzbl (%rdi,%rbp,1),%eax
mov    %r8,%rsi
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x31bf>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ae3>
and    0x8(%rcx),%rbp
mov    (%rcx),%rax
cmp    %rbx,%rbp
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3238>
mov    0x48(%rsp),%rdx
mov    %rdx,(%rcx)
mov    %rcx,%rdx
mov    0x90(%rsp),%rcx
movq   $0x1,(%rcx)
mov    %rax,0x8(%rcx)
mov    %rdx,0x10(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57fa>
lea    (%r8,%r11,8),%rax
mov    (%r8,%r11,8),%rcx
mov    0x48(%rsp),%rdx
mov    %rdx,(%r8,%r11,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x31d5>
lea    0x0(,%rbp,4),%rax
add    %rbp,%rax
mov    (%rdi,%rax,1),%ecx
movzbl 0x4(%rdi,%rax,1),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x317a>
lea    0x0(,%rbp,2),%rax
add    %rbp,%rax
movzwl (%rdi,%rax,1),%ecx
movzbl 0x2(%rdi,%rax,1),%eax
shl    $0x10,%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x317e>
mov    (%rdi,%rbp,4),%eax
mov    %r8,%rsi
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x31bf>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ae3>
movzwl (%rdi,%rbp,2),%eax
mov    %r8,%rsi
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x31bf>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ae3>
lea    0x0(,%rbp,2),%rax
add    %rbp,%rax
mov    (%rdi,%rax,2),%ecx
movzwl 0x4(%rdi,%rax,2),%eax
shl    $0x20,%rax
or     %rcx,%rax
mov    %r8,%rsi
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x31bf>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ae3>
lea    (%rdi,%rbp,8),%rax
sub    %rbp,%rax
mov    (%rax),%ecx
mov    %rdx,%rsi
movzwl 0x4(%rax),%edx
movzbl 0x6(%rax),%eax
shl    $0x20,%rdx
or     %rcx,%rdx
shl    $0x30,%rax
or     %rdx,%rax
mov    %rsi,%rdx
mov    %r8,%rsi
cmp    %rax,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ae3>
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x31f1>
mov    %rbp,%rsi
lea    (%r12,%rsi,8),%rax
mov    (%r12,%rsi,8),%rcx
mov    0x48(%rsp),%rdx
mov    %rdx,(%r12,%rsi,8)
mov    0x90(%rsp),%rdx
movq   $0x1,(%rdx)
mov    %rcx,0x8(%rdx)
mov    %rax,0x10(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57fa>
lea    -0x1(%r14),%eax
cmp    $0x5,%eax
mov    %r11,0x30(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a84>
lea    -0x20eb92(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x18f5>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    %r8,%rsi
mov    %rdi,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<1>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ad4>
movzbl (%r12,%rsi,1),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a4c>
setbe  %cl
mov    %rax,%r12
mov    0x48(%rsp),%rdx
cmovbe %rdx,%r12
cmovbe %rax,%rdx
mov    %rbp,%r15
cmova  %rbx,%r15
cmovb  %rbx,%rbp
movzbl %cl,%ebx
cmp    $0x40,%r13b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3355>
mov    %rdx,0x48(%rsp)
mov    0x8(%rsp),%rdx
addq   $0x10,0x200(%rdx)
incq   0x208(%rdx)
incq   0x218(%rdx)
mov    0x80(%rdx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x439c>
mov    (%rax),%rcx
mov    %rcx,0x80(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4456>
mov    $0x2,%eax
mov    %r9,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x34f3>
and    $0xfffffffc,%ecx
cmpb   $0xf,0x58(%rsp)
mov    $0x18,%eax
cmovne %rcx,%rax
lea    0x8(%r14),%rdx
imul   %rax,%rdx
lea    0xc(%rdx),%rsi
mov    %esi,%r12d
and    $0xfffffff0,%r12d
lea    -0x201ba1(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rdx,%rax,1),%r13d
cmp    $0xff,%r13
mov    %r8,0x40(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x398c>
mov    0x8(%rsp),%rcx
add    %r12,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3e,%r13b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b76>
mov    0x10(%rcx,%r13,8),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x417f>
mov    (%r15),%rax
mov    %rax,0x10(%rcx,%r13,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x42cc>
cmp    $0x18,%r9d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x34d1>
mov    $0x18,%eax
cmp    $0x10,%r9d
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x34e2>
lea    0x3(%r9),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x34f3>
mov    %r15,0xa0(%rsp)
mov    %rdx,0xa8(%rsp)
mov    %rbp,0xb0(%rsp)
mov    %r12,0xb8(%rsp)
movzbl %r14b,%edx
lea    0xa0(%rsp),%rcx
mov    $0x2,%r8d
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%rsi
call   ADDR <expanse_trie::mutate_map::build_map_leaf::<false>>
mov    0x60(%rsp),%rax
test   %rax,%rax
mov    0x1b0(%rsp),%r10
mov    0x38(%rsp),%r11
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3435>
shl    $0x4,%rax
add    %rsp,%rax
add    $0xf8,%rax
lea    0xf8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33d9>
nopl   0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3435>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x340b>
mov    0x8(%rsi),%rdi
lea    -0x203dd9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%r8
mov    %r8,%r9
and    %rdi,%r9
inc    %r9
xor    %rdi,%r9
and    %r8,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    (%r11),%rdi
lea    -0x1(%rdi),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33d0>
mov    %rsi,(%r10,%rdi,8)
mov    0x50(%r10),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x540a>
mov    %dl,0x48(%r10,%rdi,1)
incq   0x50(%r10)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33d0>
shl    $0x3,%ebx
mov    0x18(%rsp),%rax
add    (%rax),%rbx
mov    0x90(%rsp),%rax
movq   $0x0,(%rax)
mov    %rbx,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57fa>
lea    0x0(,%rdx,8),%edx
lea    0x20(%rdx),%rsi
mov    %esi,%r12d
and    $0xffffffe0,%r12d
lea    -0x201d38(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%r12,%rax,1),%ebp
cmp    $0xff,%rbp
mov    %r8,0x40(%rsp)
mov    %r9,0x30(%rsp)
mov    %rdx,0x70(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b95>
mov    0x8(%rsp),%rcx
add    %r12,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3e,%bpl
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c36>
mov    0x10(%rcx,%rbp,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a6e>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5be3>
xor    %eax,%eax
cmp    $0xffffffff,%r13d
sete   %al
lea    0x20(,%rax,4),%rax
xor    %ecx,%ecx
cmp    $0x19,%r9d
setae  %cl
lea    0x18(,%rcx,8),%rcx
cmp    %ecx,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3534>
mov    0x20(%rsi,%rbp,8),%r14
mov    %rsi,%r15
lea    (%r14,%r8,8),%rsi
lea    (%r14,%r8,8),%rdi
add    $0x8,%rdi
sub    %r8,%r9
shl    $0x3,%r9
mov    %r9,%rdx
mov    %r8,%r13
call   *0x16efd2(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r15,%rsi
mov    0x48(%rsp),%rax
mov    %rax,(%r14,%r13,8)
mov    0x60(%rsp),%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x385e>
lea    -0x1(%r13),%eax
test   %eax,%r13d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3551>
cmp    $0xf,%r9d
mov    0x8(%rsp),%rcx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x355c>
lea    0x4(%r9),%eax
and    $0x1c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3589>
lea    0x1(%r9),%rax
mov    0x8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3589>
cmp    $0x18,%r9d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3578>
mov    $0xc0,%r10d
mov    $0x31,%al
mov    $0x31,%r14d
mov    $0xc0,%r12d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x35b1>
xor    %eax,%eax
cmp    $0xffffffff,%r13d
sete   %al
lea    0x20(,%rax,4),%rax
lea    0x0(,%rax,8),%r10d
lea    0x8(%r10),%r12
and    $0xfffffffffffffff0,%r12
lea    -0x201e67(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%r10,%rdx,1),%r14d
cmp    $0xff,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x35f3>
mov    %r14d,%eax
add    %r12,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3e,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6bb4>
mov    0x10(%rcx,%r14,8),%r15
test   %r15,%r15
mov    %rbp,0x40(%rsp)
mov    %r9,0x28(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3674>
mov    (%r15),%rax
mov    %rax,0x10(%rcx,%r14,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x370a>
mov    %rbp,0x40(%rsp)
lock add %r12,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
cmp    $0x1,%eax
mov    %r9,0x28(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x371f>
movq   $0x0,0xa0(%rsp)
lea    0xa0(%rsp),%rdi
mov    $0x10,%esi
mov    %r10,%r12
mov    %r10,%rdx
call   *0x16eebd(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
mov    0x60(%rsp),%r14
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bcd>
mov    0xa0(%rsp),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bcd>
mov    %r15,%rdi
xor    %esi,%esi
mov    %r12,%rdx
call   *0x16eea1(%rip)        # ADDR <memset@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3741>
cmp    $0x100,%r10d
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x371f>
mov    %r10,0x20(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69c6>
mov    %rax,%r15
mov    0x8(%rsp),%rcx
mov    0x220(%rcx),%rax
mov    %rax,(%r15)
mov    %r14,0x8(%r15)
mov    %r15,0x220(%rcx)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%eax
nopl   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %r12,%rcx
lea    (%r15,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x8(%rsp),%rsi
mov    0x10(%rsi,%r14,8),%rsi
mov    %rsi,0x40(%r15,%rcx,1)
mov    0x8(%rsp),%rcx
mov    %rdx,0x10(%rcx,%r14,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x36d0>
add    $0x40,%r15
mov    0x20(%rsp),%r10
mov    %r15,%rdi
xor    %esi,%esi
mov    %r10,%rdx
call   *0x16edf8(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x60(%rsp),%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3741>
mov    0x60(%rsp),%r14
mov    $0x1,%esi
mov    %r10,%rdi
mov    %r10,%r12
call   *0x16edd3(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r15
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bcd>
test   %r13d,%r13d
mov    0x50(%rsp),%r13
mov    0x10(%rsp),%rsi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x384b>
mov    0x40(%rsp),%rax
mov    0x20(%rsi,%rax,8),%r12
lea    0x0(,%r13,8),%r14d
mov    %r15,%rdi
mov    %r12,%rsi
mov    %r14,%rdx
mov    0x16ecaa(%rip),%rbp        # ADDR <memcpy@GLIBC_2.14>
call   *%rbp
mov    %rbp,%rax
lea    (%r15,%r14,1),%rdi
add    $0x8,%rdi
add    %r12,%r14
mov    0x28(%rsp),%rbp
mov    %rbp,%rdx
sub    %r13,%rdx
shl    $0x3,%rdx
mov    %r14,%rsi
call   *%rax
test   %r12,%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x699a>
cmp    $0x3,%ebp
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37af>
mov    0x60(%rsp),%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37d1>
cmp    $0x11,%ebp
mov    0x60(%rsp),%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37c1>
add    $0x3,%ebp
and    $0x3c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37d1>
xor    %eax,%eax
cmp    $0x19,%ebp
setae  %al
lea    0x18(,%rax,8),%rbp
lea    0x0(,%rbp,8),%ecx
lea    0x8(,%rbp,8),%eax
and    $0xfffffff0,%eax
lea    -0x2020b0(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rdx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3823>
mov    0x8(%rsp),%rcx
sub    %rax,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3d,%dil
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b8b>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%r12)
mov    %r12,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3841>
mov    0x8(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
mov    %r12,%rdi
call   *0x16ebf7(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x50(%rsp),%r13
mov    0x10(%rsp),%rsi
mov    0x48(%rsp),%rax
mov    %rax,(%r15,%r13,8)
mov    0x40(%rsp),%rbp
mov    %r15,0x20(%rsi,%rbp,8)
mov    $0x1,%eax
mov    %ebx,%ecx
shl    %cl,%rax
mov    %ebx,%ecx
shr    $0x3,%ecx
and    $0x18,%ecx
or     %rax,(%rsi,%rcx,1)
mov    0x18(%rsp),%rdx
mov    0x8(%rdx),%rax
lea    0x1(%rax),%ecx
mov    %cl,0x8(%rdx)
cmpb   $0x1,0x58(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x38c6>
mov    %rdx,%rcx
shr    $0x8,%rbx
mov    0x1b0(%rsp),%rdi
mov    %rbx,0x40(%rdi)
mov    (%rcx),%rcx
mov    %rcx,0x58(%rdi)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x60(%rdi)
movzbl %al,%eax
add    $0x2,%eax
mov    %ax,0x70(%rdi)
mov    %rdx,(%rdi)
movb   $0x1,0x48(%rdi)
movq   $0x1,0x50(%rdi)
shl    $0x3,%r13d
add    0x20(%rsi,%rbp,8),%r13
test   %r14,%r14
mov    0x38(%rsp),%r10
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3974>
shl    $0x4,%r14
lea    (%rsp,%r14,1),%rax
add    $0xf8,%rax
lea    0xf8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3909>
data16 cs nopw 0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3974>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x393b>
mov    0x8(%rsi),%rdi
lea    -0x204309(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%r8
mov    %r8,%r9
and    %rdi,%r9
inc    %r9
xor    %rdi,%r9
and    %r8,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    (%r10),%rdi
lea    -0x1(%rdi),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3900>
mov    0x1b0(%rsp),%r8
mov    %rsi,(%r8,%rdi,8)
mov    0x50(%r8),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x540a>
mov    0x1b0(%rsp),%rsi
mov    %dl,0x48(%rsi,%rdi,1)
incq   0x50(%rsi)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3900>
mov    0x90(%rsp),%rax
movq   $0x0,(%rax)
mov    %r13,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57fa>
mov    0x8(%rsp),%rax
lock add %r12,0x200(%rax)
lock incq 0x208(%rax)
lock incq 0x218(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x42de>
mov    %r8,%rsi
mov    %rdi,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<5>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ad4>
lea    (%rsi,%rsi,4),%rax
mov    (%r12,%rax,1),%ecx
movzbl 0x4(%r12,%rax,1),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a7b>
mov    %r8,%rsi
mov    %rdi,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<3>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ad4>
lea    (%rsi,%rsi,2),%rax
movzwl (%r12,%rax,1),%ecx
movzbl 0x2(%r12,%rax,1),%eax
shl    $0x10,%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a7f>
mov    %r8,%rsi
mov    %rdi,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<4>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ad4>
mov    (%r12,%rsi,4),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a4c>
mov    %r8,%rsi
mov    %rdi,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<2>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ad4>
movzwl (%r12,%rsi,2),%eax
mov    0x28(%rsp),%rdx
cmp    %rax,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ac2>
mov    %r8,%rsi
mov    %rdi,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<6>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ad4>
lea    (%rsi,%rsi,2),%rax
mov    (%r12,%rax,2),%ecx
movzwl 0x4(%r12,%rax,2),%eax
shl    $0x20,%rax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3aba>
mov    %r8,%rsi
mov    %rdi,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<7>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ad4>
lea    (%r12,%rsi,8),%rax
sub    %rsi,%rax
mov    (%rax),%ecx
movzwl 0x4(%rax),%edx
movzbl 0x6(%rax),%eax
shl    $0x20,%rdx
or     %rcx,%rdx
shl    $0x30,%rax
or     %rdx,%rax
mov    0x28(%rsp),%rdx
cmp    %rdx,%rax
mov    0x10(%rsp),%r12
mov    0x30(%rsp),%r11
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x31c4>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ae3>
mov    0x10(%rsp),%r12
mov    0x30(%rsp),%r11
mov    0x28(%rsp),%rdx
mov    %rsi,0x40(%rsp)
cmp    $0x1,%r14
sete   %r10b
cmp    $0x1,%r14d
mov    $0x19,%eax
mov    $0x20,%ecx
cmovne %rcx,%rax
cmp    %rax,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3b6b>
lea    0x2(%rbp),%r9
test   %rbp,%rbp
mov    %r9,0xe0(%rsp)
jle    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3dce>
cmp    $0xf,%rbp
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e09>
lea    0x5(%rbp),%eax
and    $0x1c,%eax
cmp    $0x2,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f58>
lea    0x8(%r14),%r9
lea    (%r9,%r9,1),%ecx
mov    %rcx,0x70(%rsp)
mov    0x50(%rsp),%rdx
mov    (%rdx),%ecx
mov    0x3(%rdx),%edx
mov    %edx,0x8b(%rsp)
mov    %ecx,0x88(%rsp)
mov    $0x2,%r13d
mov    0x8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44dd>
mov    %rax,0x30(%rsp)
mov    0x50(%rsp),%r12
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %ecx,0x8b(%rsp)
mov    %eax,0x88(%rsp)
lea    0xc8(%rsp),%rdi
mov    0x18(%rsp),%rsi
mov    %r14d,%edx
mov    %r8,%rcx
mov    %r8,0x70(%rsp)
call   *0x16ebee(%rip)        # ADDR <_DYNAMIC+0x590>
mov    0x18(%rsp),%rax
mov    (%rax),%rax
mov    %rax,0x20(%rsp)
and    $0xfffffffffffffffc,%rbp
add    $0x4,%rbp
cmpq   $0x21,0x70(%rsp)
mov    $0x20,%eax
cmovb  %rax,%rbp
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %ecx,0xf3(%rsp)
mov    %eax,0xf0(%rsp)
mov    0xd8(%rsp),%rsi
cmp    0x40(%rsp),%rsi
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6950>
mov    %rsi,0x10(%rsp)
cmp    0xc8(%rsp),%rsi
mov    0x1b0(%rsp),%r12
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3c1d>
lea    0xc8(%rsp),%rdi
call   *0x16f12b(%rip)        # ADDR <_DYNAMIC+0xb40>
mov    0x40(%rsp),%rax
mov    %rax,%rcx
shl    $0x4,%rcx
add    0xd0(%rsp),%rcx
mov    0x10(%rsp),%rdx
sub    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3c58>
lea    0x10(%rcx),%rdi
shl    $0x4,%rdx
mov    %rcx,0x40(%rsp)
mov    0x40(%rsp),%rsi
call   *0x16e89d(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x40(%rsp),%rcx
mov    0x10(%rsp),%rax
lea    0x1(%rax),%r8
mov    0x28(%rsp),%rax
mov    %rax,(%rcx)
mov    0x48(%rsp),%rax
mov    %rax,0x8(%rcx)
mov    %r8,0xd8(%rsp)
mov    0x58(%rsp),%rax
cmp    %r14b,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d50>
mov    %r14,%rsi
xor    $0x7,%rsi
sub    %r14b,%al
movzbl %al,%edi
dec    %rdi
lea    -0x4(%r13),%eax
movzbl %al,%eax
mov    0x18(%rsp),%rcx
add    %rax,%rcx
add    $0x7,%rcx
xor    %eax,%eax
lea    0x163c6c(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    %rsi,%rdi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b56>
mov    %rax,%r9
shl    $0x8,%r9
movzbl 0x1(%rcx,%rdi,1),%eax
or     %r9,%rax
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3cc0>
mov    %r15d,%ecx
shl    %cl,%rax
mov    0xd0(%rsp),%rcx
mov    %r8,%rsi
mov    %rcx,%rdx
and    $0x7,%rsi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d0c>
mov    %rcx,%rdx
nopl   0x0(%rax,%rax,1)
or     %rax,(%rdx)
add    $0x10,%rdx
dec    %rsi
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d00>
cmpq   $0x7,0x10(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d48>
shl    $0x4,%r8
add    %r8,%rcx
nopl   0x0(%rax,%rax,1)
or     %rax,(%rdx)
or     %rax,0x10(%rdx)
or     %rax,0x20(%rdx)
or     %rax,0x30(%rdx)
or     %rax,0x40(%rdx)
or     %rax,0x50(%rdx)
or     %rax,0x60(%rdx)
or     %rax,0x70(%rdx)
sub    $0xffffffffffffff80,%rdx
cmp    %rcx,%rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d20>
mov    0xd8(%rsp),%r8
mov    0xd0(%rsp),%rcx
cmp    0x30(%rsp),%r8
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d80>
cmp    $0x1,%r14d
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e22>
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%rsi
mov    %rcx,%rdx
mov    %r8,%rcx
call   ADDR <expanse_trie::mutate_map::build_bitmap_leaf_map::<false>>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d92>
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%rsi
mov    %r14d,%edx
call   ADDR <expanse_trie::mutate_map::build_map_leaf::<false>>
cmp    %r14b,0x58(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x55e6>
mov    0x58(%rsp),%rax
movzbl %al,%edx
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69d6>
sub    %r14,%rdx
lea    (%rsp,%r14,1),%rsi
add    $0xf0,%rsi
mov    0x50(%rsp),%rdi
add    %r14,%rdi
call   *0x16e657(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x55e6>
lea    0x8(%r14),%rsi
mov    %r8,%rax
imul   %rsi,%rax
mov    %rax,0x70(%rsp)
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x8b(%rsp)
mov    %eax,0x88(%rsp)
mov    %r8,%r13
mov    %r9,%rax
mov    %rsi,%r9
mov    0x8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44dd>
mov    %rdx,%rsi
xor    %eax,%eax
cmp    $0x17,%rbp
setae  %al
lea    0x18(,%rax,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f5b>
shl    $0x4,%r8
mov    -0x10(%rcx,%r8,1),%rdx
xor    (%rcx),%rdx
mov    0x58(%rsp),%rax
cmp    $0x7,%al
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e56>
lea    0x0(,%rax,8),%eax
mov    $0xffffffffffffffff,%rdi
mov    %rcx,%rsi
mov    %eax,%ecx
shl    %cl,%rdi
mov    %rsi,%rcx
not    %rdi
and    %rdi,%rdx
mov    $0x7f,%eax
bsr    %rdx,%rax
xor    $0x3f,%rax
mov    %eax,%edx
and    $0xfffffff8,%edx
cmp    $0x38,%edx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x40a5>
add    %rcx,%r8
lea    0xa0(%rsp),%rdi
mov    %rcx,%rsi
mov    %r8,%rdx
call   ADDR <<alloc::vec::Vec<(u64, u64)> as alloc::vec::spec_from_iter::SpecFromIter<(u64, u64), core::iter::adapters::map::Map<core::slice::iter::Iter<(u64, u64)>, expanse_trie::mutate_map::map_insert_with_path_occ<false, true, false>::{closure#0}>>>::from_iter>
cmpq   $0x0,0xd8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c12>
mov    0xd0(%rsp),%rax
mov    (%rax),%rax
mov    %rax,0x50(%rsp)
mov    0xa8(%rsp),%rdx
mov    0xb0(%rsp),%rcx
mov    0x8(%rsp),%rdi
mov    0x18(%rsp),%r13
mov    %r13,%rsi
call   ADDR <expanse_trie::mutate_map::build_bitmap_leaf_map::<false>>
mov    0x58(%rsp),%r15
movzbl %r15b,%edx
mov    %r13,%rdi
mov    $0x1,%esi
mov    0x50(%rsp),%rcx
call   *0x16e8bc(%rip)        # ADDR <_DYNAMIC+0x598>
cmp    $0x1,%r15b
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f36>
mov    %rbx,%rax
shr    $0x8,%rax
mov    %rax,0x40(%r12)
mov    0x18(%rsp),%rcx
mov    (%rcx),%rax
mov    %rax,0x58(%r12)
movq   $0x0,0x60(%r12)
movzwl 0xd8(%rsp),%eax
mov    %ax,0x70(%r12)
mov    %rcx,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
cmpq   $0x0,0xa0(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x55e6>
mov    0xa8(%rsp),%rdi
call   *0x16e4e5(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x55e6>
mov    %rdx,%rsi
xor    %ecx,%ecx
cmp    $0x19,%r8
setae  %cl
movabs $0x7ffffffffffffff0,%rdx
or     $0xc,%rdx
and    %rbp,%rdx
add    $0x4,%rdx
cmp    $0x11,%r8
lea    0x18(,%rcx,8),%rcx
cmovae %rcx,%rdx
cmp    %rdx,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x404d>
add    $0x4,%ebp
and    $0x3c,%ebp
cmp    $0x11,%r8
cmovb  %ebp,%ecx
lea    (%r12,%rcx,8),%rdi
lea    -0x1(%r14),%eax
cmp    $0x5,%eax
mov    %rsi,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x603c>
lea    -0x20f92a(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x190d>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    %r8,%r15
mov    0x40(%rsp),%r13
sub    %r13,%r15
mov    0x38(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4036>
lea    (%r12,%r13,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r12,%r13,8),%rdi
add    $0x8,%rdi
lea    0x0(,%r15,8),%rdx
mov    0x16e4fb(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    %r11,0x30(%rsp)
mov    %r8,0x70(%rsp)
call   *%r12
mov    0x8(%rsp),%rax
lea    (%rax,%r13,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r13,1),%rdi
inc    %rdi
mov    %r15,%rdx
call   *%r12
mov    0x8(%rsp),%rdi
mov    0x70(%rsp),%r8
mov    0x28(%rsp),%rdx
mov    0x30(%rsp),%r11
mov    0x10(%rsp),%r12
mov    0x48(%rsp),%rax
mov    %rax,(%r12,%r13,8)
mov    %dl,(%rdi,%r13,1)
mov    0x18(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x659f>
cmp    $0x11,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x40f8>
lea    0x4(%rbp),%esi
and    $0x3c,%esi
lea    0x8(%r14),%rax
mov    %rax,%rdi
imul   %rax,%rsi
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x8b(%rsp)
mov    %eax,0x88(%rsp)
cmp    $0xe,%rbp
mov    %rsi,0x70(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44af>
lea    0x5(%rbp),%eax
and    $0x1c,%eax
mov    %r8,%r13
mov    0x8(%rsp),%rcx
mov    %rdi,%r9
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44dd>
mov    $0x3f,%cl
sub    %al,%cl
shr    $0x3,%cl
inc    %cl
mov    0x58(%rsp),%rax
cmp    $0x8,%al
movzbl %cl,%r15d
movzbl %al,%eax
mov    %eax,0x48(%rsp)
cmovae %eax,%r15d
mov    0x8(%rsp),%rdx
addq   $0x40,0x200(%rdx)
incq   0x208(%rdx)
incq   0x218(%rdx)
mov    0x18(%rdx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x543d>
mov    (%rax),%rcx
mov    %rcx,0x18(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54ac>
lea    0x8(%r14),%r9
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x8b(%rsp)
mov    %eax,0x88(%rsp)
cmp    $0x18,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44b9>
lea    0x0(,%r9,8),%eax
lea    (%rax,%rax,2),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44bf>
mov    0x48(%rsp),%rax
mov    %rax,(%rsi)
mov    0x98(%rsp),%r12
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %ecx,0x7b(%rsp)
mov    %eax,0x78(%rsp)
mov    0x38(%rsp),%r10
mov    0x10(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6273>
lea    -0x20fb44(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x18a5>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %bl,0x78(%rsp,%r11,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6273>
cmp    $0x100,%edx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x42de>
mov    %rsi,0x30(%rsp)
mov    %rdx,0x20(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69c6>
mov    %rax,%r15
mov    0x8(%rsp),%rbp
mov    0x220(%rbp),%rax
mov    %rax,(%r15)
mov    %r13,0x8(%r15)
mov    %r15,0x220(%rbp)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x420c>
mov    %rcx,%rax
dec    %rax
mov    %rax,%rsi
imul   %r12,%rsi
lea    (%r15,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%rbp,%r13,8),%r8
mov    %r8,0x40(%r15,%rsi,1)
mov    %rdi,0x10(%rbp,%r13,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x41e6>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x42c3>
lea    -0x4(%rax),%rcx
mov    0x30(%rsp),%r9
shr    $0x4,%r9d
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
mov    %r15,%r9
lea    (%r9,%r8,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x10(%rbp,%r13,8)
lea    (%r9,%rdi,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%rbp,%r13,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%rbp,%r13,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%rbp,%r13,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x426e>
add    $0x40,%r15
mov    0x20(%rsp),%rdx
mov    %r15,%rdi
xor    %esi,%esi
call   *0x16e239(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x38(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x42ff>
mov    $0x10,%esi
mov    %rdx,%rdi
mov    %rdx,%r12
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%r15
test   %rax,%rax
mov    0x38(%rsp),%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bcd>
mov    0x58(%rsp),%r12
lea    0x1(%r12),%r8
lea    0x20(,%r8,8),%eax
and    $0xffffffe0,%eax
cmp    $0xf,%r12b
mov    $0xc0,%r13d
cmovne %rax,%r13
add    %r15,%r13
mov    0x50(%rsp),%r10
test   %r10,%r10
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4369>
lea    0x0(,%r10,8),%rdx
mov    %r15,%rdi
mov    0x40(%rsp),%rsi
call   *0x16e0db(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x50(%rsp),%rdx
imul   %r14,%rdx
mov    %r13,%rdi
mov    0x98(%rsp),%rsi
call   *0x16e0c1(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x50(%rsp),%r10
lea    0x1(%r12),%r8
mov    0x48(%rsp),%rax
mov    %rax,(%r15,%r10,8)
mov    0x10(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x583b>
lea    -0x20fd99(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x186d>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %bl,0x0(%r13,%r10,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5881>
mov    %rdi,0x98(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69c6>
mov    0x8(%rsp),%r8
mov    0x220(%r8),%rcx
mov    %rcx,(%rax)
movq   $0xe,0x8(%rax)
mov    %rax,0x220(%r8)
lea    0xff0(%rax),%rcx
mov    $0xffffffffffffff08,%rdx
mov    0x80(%r8),%rsi
mov    %rsi,(%rcx)
mov    %rcx,0x80(%r8)
lea    -0x10(%rcx),%rsi
mov    0x80(%r8),%rdi
mov    %rdi,-0x10(%rcx)
mov    %rsi,0x80(%r8)
lea    -0x20(%rcx),%rsi
mov    0x80(%r8),%rdi
mov    %rdi,-0x20(%rcx)
mov    %rsi,0x80(%r8)
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x444a>
lea    -0x30(%rcx),%rsi
mov    0x80(%r8),%rdi
mov    %rdi,-0x30(%rcx)
mov    %rsi,0x80(%r8)
add    $0xffffffffffffffc0,%rcx
add    $0x4,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x43e8>
add    $0x40,%rax
mov    0x98(%rsp),%rdi
mov    0x48(%rsp),%rcx
mov    %rcx,(%rax)
mov    %r12,0x8(%rax)
movw   $0x0,0x6c(%rsp)
movl   $0x0,0x68(%rsp)
movzbl %r14b,%ecx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6102>
lea    -0x20fe43(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0x18c1>
movslq (%rdx,%rcx,4),%rcx
add    %rdx,%rcx
mov    0x1b0(%rsp),%r11
mov    0x60(%rsp),%rdx
jmp    *%rcx
mov    %r15b,0x68(%rsp)
mov    %bpl,0x69(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x614d>
mov    0x8(%rsp),%rcx
mov    %rdi,%r9
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44c9>
mov    %r9d,%eax
shl    $0x5,%eax
mov    %rax,0x70(%rsp)
mov    0x8(%rsp),%rcx
xor    %eax,%eax
cmp    $0x17,%rbp
setae  %al
lea    0x18(,%rax,8),%rax
mov    %r8,%r13
imul   %rax,%r9
lea    0xf(%r9),%r12
and    $0xfffffffffffffff0,%r12
cmp    $0x178,%r9
mov    %r11,0x30(%rsp)
mov    %r10b,0xe8(%rsp)
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x454e>
lea    -0x202dcd(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%r9,%rax,1),%r15d
cmp    $0xff,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4590>
add    %r12,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3e,%r15b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6bc9>
mov    0x10(%rcx,%r15,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x460a>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%r15,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x469a>
lock add %r12,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
mov    $0x1,%esi
mov    %r9,%rdi
mov    %r9,%r15
call   *0x16df91(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r10
test   %rax,%rax
mov    0x1b0(%rsp),%r12
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x46b3>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c04>
lock add %r12,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
cmp    $0xf,%r9
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4566>
movq   $0x0,0xa0(%rsp)
lea    0xa0(%rsp),%rdi
mov    $0x10,%esi
mov    %r9,%rdx
mov    %r9,%r15
call   *0x16df2d(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
mov    0x1b0(%rsp),%r12
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c04>
mov    0xa0(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c04>
xor    %esi,%esi
mov    %r15,%rdx
mov    %rdi,%r15
call   *0x16df0e(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r15,%r10
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x46b3>
cmp    $0x100,%r9
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4566>
mov    %r9,0x20(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69c6>
mov    %rax,%rdi
mov    0x8(%rsp),%rcx
mov    0x220(%rcx),%rax
mov    %rax,(%rdi)
mov    %r15,0x8(%rdi)
mov    %rdi,0x220(%rcx)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%eax
xchg   %ax,%ax
dec    %rax
mov    %rax,%rcx
imul   %r12,%rcx
lea    (%rdi,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x8(%rsp),%rsi
mov    0x10(%rsi,%r15,8),%rsi
mov    %rsi,0x40(%rdi,%rcx,1)
mov    0x8(%rsp),%rcx
mov    %rdx,0x10(%rcx,%r15,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4660>
add    $0x40,%rdi
mov    0x20(%rsp),%r9
xor    %esi,%esi
mov    %r9,%rdx
mov    %rdi,%r15
call   *0x16de68(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r15,%r10
mov    0x1b0(%rsp),%r12
lea    -0x1(%r14),%eax
cmp    $0x5,%eax
mov    %r10,0x20(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49c4>
lea    -0x210024(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x1925>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
mov    0x48(%rsp),%r14
mov    0x10(%rsp),%r8
jmp    *%rax
mov    0x40(%rsp),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4707>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16dd23(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    %r13,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4733>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16dcf7(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ad7>
lea    (%r8,%r13,8),%r14
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4afd>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4dca>
mov    0x40(%rsp),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4780>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16dcaa(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    %r13,%rdx
sub    %r15,%rdx
mov    %r15,%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47af>
lea    (%r10,%r14,8),%rdi
lea    (%r8,%r14,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16dc7b(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a4a>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a6e>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c03>
mov    0x40(%rsp),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47fc>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16dc2e(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    %r13,%rdx
sub    %r15,%rdx
mov    %r15,%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x482b>
lea    (%r10,%r14,8),%rdi
lea    (%r8,%r14,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16dbff(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a79>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a9d>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c9f>
mov    0x40(%rsp),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4878>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16dbb2(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    %r13,%rdx
sub    %r15,%rdx
mov    %r15,%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x48a7>
lea    (%r10,%r14,8),%rdi
lea    (%r8,%r14,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16db83(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4aa8>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4acc>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d36>
mov    0x40(%rsp),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x48f4>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16db36(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    %r13,%rdx
sub    %r15,%rdx
mov    %r15,%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4923>
lea    (%r10,%r14,8),%rdi
lea    (%r8,%r14,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16db07(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b08>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b2c>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e4c>
mov    0x40(%rsp),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4970>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16daba(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    %r13,%rdx
sub    %r15,%rdx
mov    %r15,%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x499f>
lea    (%r10,%r14,8),%rdi
lea    (%r8,%r14,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16da8b(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b37>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b5b>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4edb>
mov    0x40(%rsp),%r15
test   %r15,%r15
mov    0x48(%rsp),%r14
mov    0x10(%rsp),%r8
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49f6>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16da34(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    %r13,%rdx
sub    %r15,%rdx
mov    %r15,%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a25>
lea    (%r10,%r14,8),%rdi
lea    (%r8,%r14,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16da05(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b66>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b8a>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f7f>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bdb>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bf2>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c03>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c77>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c8e>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c9f>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d0e>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d25>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d36>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4da2>
lea    0x20(,%rbp,8),%r14d
and    $0x1e0,%r14d
add    %r8,%r14
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4db9>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4dca>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e24>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e3b>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e4c>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4eb3>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4eca>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4edb>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f57>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f6e>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f7f>
mov    0x8(%rsp),%rax
lock add %r12,0x200(%rax)
lock incq 0x208(%rax)
lock incq 0x218(%rax)
mov    $0x10,%esi
mov    %r12,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,0x20(%rsp)
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5bf3>
mov    $0x10,%edi
mov    %r12,%rsi
call   *0x16d8ad(%rip)        # ADDR <_DYNAMIC+0x280>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%rbp
lea    (%r10,%rbp,8),%r15
test   %r14,%r14
mov    %rsi,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c33>
mov    %r14,%rbp
lea    (%r14,%r14,4),%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16d7fc(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,%rsi
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c39>
mov    %r14,%rsi
xor    %r14d,%r14d
mov    0x38(%rsp),%rbp
mov    0x28(%rsp),%rdx
mov    %rdx,%rcx
shr    $0x20,%rcx
mov    %r13,%rax
mov    %edx,(%r15,%r14,1)
mov    %cl,0x4(%r15,%r14,1)
sub    %rsi,%rax
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5018>
add    %r14,%r15
mov    0x48(%rsp),%rsi
add    %r14,%rsi
add    $0x5,%r15
lea    (%rax,%rax,4),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5005>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%rbp
lea    (%r10,%rbp,8),%r15
test   %r14,%r14
mov    %rsi,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ccf>
mov    %r14,%rbp
lea    (%r14,%r14,2),%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16d760(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,%rsi
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4cd5>
mov    %r14,%rsi
xor    %r14d,%r14d
mov    0x38(%rsp),%rbp
mov    0x28(%rsp),%rdx
mov    %edx,%ecx
shr    $0x10,%ecx
mov    %r13,%rax
mov    %dx,(%r15,%r14,1)
mov    %cl,0x2(%r15,%r14,1)
sub    %rsi,%rax
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5018>
add    %r14,%r15
mov    0x48(%rsp),%rsi
add    %r14,%rsi
add    $0x3,%r15
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f4e>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%rbp
lea    (%r10,%rbp,8),%r15
test   %r14,%r14
mov    %rsi,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d6a>
mov    %r14,%rbp
lea    0x0(,%r14,4),%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16d6c5(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,%rcx
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d70>
mov    %r14,%rcx
xor    %r14d,%r14d
mov    0x38(%rsp),%rbp
mov    %r13,%rdx
mov    0x28(%rsp),%rax
mov    %eax,(%r15,%r14,1)
sub    %rcx,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5018>
add    %r14,%r15
mov    0x48(%rsp),%rsi
add    %r14,%rsi
add    $0x4,%r15
shl    $0x2,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5005>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%r14
add    $0xc0,%r14
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%rbp
lea    (%r10,%rbp,8),%r15
mov    0x40(%rsp),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4df1>
mov    %r15,%rdi
mov    %r14,%rsi
mov    %rbp,%rdx
call   *0x16d639(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
mov    %r13,%rdx
mov    0x28(%rsp),%rax
mov    %al,(%r15,%rbp,1)
sub    %rbp,%rdx
mov    0x38(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5018>
mov    0x40(%rsp),%rax
add    %rax,%r15
add    %rax,%r14
inc    %r15
mov    %r15,%rdi
mov    %r14,%rsi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5008>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%rbp
lea    (%r10,%rbp,8),%r15
test   %r14,%r14
mov    %rsi,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e7b>
mov    %r14,%rbp
add    %r14,%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16d5b4(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,%rcx
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e81>
mov    %r14,%rcx
xor    %r14d,%r14d
mov    0x38(%rsp),%rbp
mov    %r13,%rdx
mov    0x28(%rsp),%rax
mov    %ax,(%r15,%r14,1)
sub    %rcx,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5018>
add    %r14,%r15
mov    0x48(%rsp),%rsi
add    %r14,%rsi
add    $0x2,%r15
add    %rdx,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5005>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%rbp
lea    (%r10,%rbp,8),%r15
test   %r14,%r14
mov    %rsi,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f0f>
lea    (%r14,%r14,1),%rax
mov    %r14,%rbp
lea    (%rax,%rax,2),%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16d520(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,%rsi
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f15>
mov    %r14,%rsi
xor    %r14d,%r14d
mov    0x38(%rsp),%rbp
mov    0x28(%rsp),%rdx
mov    %rdx,%rcx
shr    $0x20,%rcx
mov    %r13,%rax
mov    %edx,(%r15,%r14,1)
mov    %cx,0x4(%r15,%r14,1)
sub    %rsi,%rax
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5018>
add    %r14,%r15
mov    0x48(%rsp),%rsi
add    %r14,%rsi
add    $0x6,%r15
add    %rax,%rax
lea    (%rax,%rax,2),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5005>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%rbp
lea    (%r10,%rbp,8),%r15
test   %r14,%r14
mov    %rsi,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4fb6>
mov    %r14,%rbp
lea    0x0(,%r14,8),%r14
sub    %rbp,%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16d479(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,%rdi
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4fbc>
mov    %r14,%rdi
xor    %r14d,%r14d
mov    0x38(%rsp),%rbp
mov    0x28(%rsp),%rsi
mov    %rsi,%rcx
shr    $0x20,%rcx
mov    %rsi,%rdx
shr    $0x30,%rdx
mov    %r13,%rax
mov    %esi,(%r15,%r14,1)
mov    %cx,0x4(%r15,%r14,1)
mov    %dl,0x6(%r15,%r14,1)
sub    %rdi,%rax
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5018>
add    %r14,%r15
mov    0x48(%rsp),%rsi
add    %r14,%rsi
add    $0x7,%r15
lea    0x0(,%rax,8),%rdx
sub    %rax,%rdx
mov    %r15,%rdi
call   *0x16d412(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%r8
mov    0x18(%rsp),%rdx
mov    %r10,(%rdx)
mov    0x88(%rsp),%eax
mov    0x8b(%rsp),%ecx
mov    0x50(%rsp),%rsi
mov    %ecx,0x3(%rsi)
mov    %eax,(%rsi)
mov    0x8(%rdx),%rax
xor    %rax,%r13
and    0x30(%rsp),%r13
xor    %rax,%r13
mov    %r13,0x8(%rdx)
test   %r8,%r8
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6937>
mov    0x70(%rsp),%rsi
lea    0xf(%rsi),%rax
and    $0xfffffffffffffff0,%rax
cmp    $0x177,%rsi
mov    0x60(%rsp),%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52e5>
mov    %rdx,%rcx
lea    -0x203944(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rsi,%rdx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52e5>
mov    0x8(%rsp),%rdx
sub    %rax,0x200(%rdx)
decq   0x208(%rdx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b8b>
mov    0x10(%rdx,%rdi,8),%rax
mov    %rax,(%r8)
mov    %r8,0x10(%rdx,%rdi,8)
cmpb   $0x1,0x58(%rsp)
sete   %al
test   %al,0xe8(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5322>
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5372>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52b3>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52b3>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5131>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205aff(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52b3>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5171>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205b3f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52b3>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x51b1>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205b7f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52b3>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x51f1>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205bbf(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52b3>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5231>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205bff(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52b3>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x526d>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205c3b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52b3>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52a9>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205c77(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
xor    %edx,%edx
test   %r14,%r14
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x537d>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x541c>
mov    0x8(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
mov    %r8,%rdi
call   *0x16d135(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%rcx
cmpb   $0x1,0x58(%rsp)
sete   %al
test   %al,0xe8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x50cd>
shr    $0x8,%rbx
mov    %rbx,0x40(%r12)
movq   $0x0,0x58(%r12)
mov    %r10,0x60(%r12)
mov    0xe0(%rsp),%rax
mov    %ax,0x70(%r12)
mov    %rcx,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
mov    $0x1,%edx
test   %r14,%r14
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x537d>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x541c>
xor    %edx,%edx
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x541c>
shl    $0x4,%r14
lea    (%rsp,%r14,1),%rax
add    $0xf8,%rax
lea    0xf8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x53bb>
data16 cs nopw 0x0(%rax,%rax,1)
mov    %sil,0x48(%r12,%rdi,1)
mov    0x50(%r12),%rdx
inc    %rdx
mov    %rdx,0x50(%r12)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x541c>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x53f1>
mov    0x8(%rdi),%rdx
lea    -0x205dbb(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rsi,8),%r8
mov    %r8,%r9
and    %rdx,%r9
inc    %r9
xor    %rdx,%r9
and    %r8,%r9
xor    %rdx,%r9
mov    %r9,0x8(%rdi)
mov    0x0(%rbp),%rdx
lea    -0x1(%rdx),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x53b2>
mov    %rdi,(%r12,%rdx,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x53a0>
lea    0x16a667(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
mov    $0x8,%esi
call   *0x16cff4(%rip)        # ADDR <_DYNAMIC+0x208>
mov    0x40(%rsp),%rax
lea    (%r10,%rax,8),%rax
mov    0x90(%rsp),%rcx
movq   $0x0,(%rcx)
mov    %rax,0x10(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57fa>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6aba>
mov    0x8(%rsp),%r8
mov    0x220(%r8),%rcx
mov    %rcx,(%rax)
movq   $0x1,0x8(%rax)
mov    %rax,0x220(%r8)
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
jg     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x547f>
add    $0x40,%rax
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rax)
movdqu %xmm0,0x30(%rax)
movdqu %xmm0,0x20(%rax)
movdqu %xmm0,0x10(%rax)
mov    %r15b,0x5(%rax)
movl   $0x0,0xa3(%rsp)
movl   $0x0,0xa0(%rsp)
mov    0x18(%rsp),%rdx
mov    %rax,(%rdx)
mov    0xa0(%rsp),%eax
mov    0xa3(%rsp),%ecx
mov    0x50(%rsp),%rsi
mov    %ecx,0x3(%rsi)
mov    %eax,(%rsi)
movb   $0x1,0xf(%rdx)
movzbl %r15b,%r13d
cmp    0x58(%rsp),%r15b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5538>
cmpq   $0x0,0xd8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c4b>
mov    0xd0(%rsp),%rax
mov    (%rax),%rcx
mov    0x18(%rsp),%rdi
mov    %r13d,%esi
mov    0x48(%rsp),%edx
call   *0x16d268(%rip)        # ADDR <_DYNAMIC+0x598>
mov    0xd8(%rsp),%r12
test   %r12,%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x55b3>
mov    %r13,0x10(%rsp)
mov    0xd0(%rsp),%r13
shl    $0x4,%r12
add    %r13,%r12
lea    0xa0(%rsp),%r15
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    0x0(%r13),%rcx
mov    0x8(%r13),%r8
mov    0x1b0(%rsp),%rax
mov    %rax,(%rsp)
mov    %r15,%rdi
mov    0x8(%rsp),%rsi
mov    0x18(%rsp),%rdx
mov    0x48(%rsp),%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
add    $0x10,%r13
cmp    %r12,%r13
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5570>
mov    0xd8(%rsp),%rax
mov    0x10(%rsp),%r13
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x55b5>
xor    %eax,%eax
cmp    $0x8,%r13b
mov    0x1b0(%rsp),%r12
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c25>
dec    %rax
mov    0x50(%rsp),%rsi
mov    (%rsi),%rcx
xor    %rcx,%rax
lea    -0x205fc4(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rdx,%r13,8),%rax
xor    %rcx,%rax
mov    %rax,(%rsi)
cmpq   $0x0,0x20(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6967>
lea    0x8(%r14),%rax
imul   %rax,%rbp
lea    0xc(%rbp),%rax
and    $0xfffffffffffffff0,%rax
cmp    $0x177,%rbp
mov    0x8(%rsp),%rcx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5670>
lea    -0x203ede(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl 0x0(%rbp,%rdx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x56c6>
sub    %rax,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
mov    0x38(%rsp),%r14
mov    0x58(%rsp),%r15
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6bde>
mov    0x10(%rcx,%rdi,8),%rax
mov    0x20(%rsp),%rdx
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
mov    0x60(%rsp),%rax
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x56f9>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5787>
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%rbp
mov    0x38(%rsp),%r14
mov    0x58(%rsp),%r15
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x56e0>
lea    -0x20611b(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0x16a139(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x1632c2(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0xc7(%rsp),%rdx
mov    $0x11,%esi
call   *0x16ce0f(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c5c>
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
mov    0x38(%rsp),%r14
mov    0x58(%rsp),%r15
mov    0x20(%rsp),%rdi
call   *0x16cd4d(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x60(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5787>
shl    $0x4,%rax
add    %rsp,%rax
add    $0xf8,%rax
lea    0xf8(%rsp),%rcx
lea    0x16a363(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5729>
nopw   0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5787>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x575b>
mov    0x8(%rdi),%r8
lea    -0x206129(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%r9
mov    %r9,%r10
and    %r8,%r10
inc    %r10
xor    %r8,%r10
and    %r9,%r10
xor    %r8,%r10
mov    %r10,0x8(%rdi)
mov    (%r14),%r8
lea    -0x1(%r8),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5720>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b51>
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5720>
mov    0x16cf62(%rip),%rax        # ADDR <_DYNAMIC+0x4e8>
movzbl (%rax),%eax
cmp    $0x2,%al
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57a8>
movzbl %r15b,%edx
mov    0x18(%rsp),%rdi
mov    %rbx,%rsi
call   ADDR <expanse_trie::get::locate_slot_popcnt>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57c5>
movzbl %al,%eax
cmp    $0x1,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69b3>
movzbl %r15b,%edx
mov    0x18(%rsp),%rdi
mov    %rbx,%rsi
call   ADDR <expanse_trie::get::locate_slot_swar>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x697c>
mov    0x90(%rsp),%rcx
movq   $0x0,(%rcx)
mov    %rax,0x10(%rcx)
cmpq   $0x0,0xc8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57fa>
mov    0xd0(%rsp),%rdi
call   *0x16cc3e(%rip)        # ADDR <free@GLIBC_2.2.5>
add    $0x178,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
lea    (%r10,%r10,4),%rax
mov    %ebx,0x0(%r13,%rax,1)
shr    $0x20,%rbx
mov    %bl,0x4(%r13,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5881>
lea    (%r10,%r10,2),%rax
mov    %bx,0x0(%r13,%rax,1)
shr    $0x10,%ebx
mov    %bl,0x2(%r13,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5881>
mov    %ebx,0x0(%r13,%r10,4)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5881>
mov    %rbx,0x0(%r13,%r10,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5881>
mov    %bx,0x0(%r13,%r10,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5881>
lea    (%r10,%r10,2),%rax
mov    %ebx,0x0(%r13,%rax,2)
shr    $0x20,%rbx
mov    %bx,0x4(%r13,%rax,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5881>
lea    0x0(,%r10,8),%rax
add    %r13,%rax
sub    %r10,%rax
mov    %ebx,(%rax)
mov    %rbx,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%rbx
mov    %bl,0x6(%rax)
mov    %r8,%rbx
sub    %r10,%rbx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x58e1>
mov    0x40(%rsp),%rax
lea    (%rax,%r10,8),%rsi
lea    0x1(%r10),%r12
lea    (%r15,%r10,8),%rdi
add    $0x8,%rdi
lea    0x0(,%rbx,8),%edx
call   *0x16cb75(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x50(%rsp),%rsi
imul   %r14,%rsi
add    0x98(%rsp),%rsi
imul   %r14,%r12
add    %r12,%r13
imul   %r14,%rbx
mov    %r13,%rdi
mov    %rbx,%rdx
call   *0x16cb4d(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x50(%rsp),%r10
mov    0x58(%rsp),%rax
lea    0x1(%rax),%r8
mov    0x18(%rsp),%rax
mov    (%rax),%rdi
test   %rdi,%rdi
mov    0x1b0(%rsp),%rbx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69f3>
lea    0x0(,%r8,8),%eax
add    $0x18,%eax
and    $0xffffffe0,%eax
cmpb   $0x1,0x58(%rsp)
mov    $0x10,%ecx
cmovne %rax,%rcx
lea    -0x2041e4(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rax,1),%eax
cmp    $0xff,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5956>
mov    0x8(%rsp),%rdx
sub    %rcx,0x200(%rdx)
decq   0x208(%rdx)
cmp    $0x3e,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6bef>
mov    %rdx,%rcx
mov    0x10(%rcx,%rax,8),%rcx
mov    %rcx,(%rdi)
mov    %rdi,0x10(%rdx,%rax,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x597b>
mov    0x8(%rsp),%rax
lock sub %rcx,0x200(%rax)
lock decq 0x208(%rax)
call   *0x16cac7(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x50(%rsp),%r10
mov    0x28(%rsp),%r8
mov    0x98(%rsp),%rdi
lea    0x4(%r14),%eax
movl   $0x0,0xa3(%rsp)
movl   $0x0,0xa0(%rsp)
mov    0x18(%rsp),%rsi
mov    %r15,(%rsi)
mov    0xa0(%rsp),%ecx
mov    0xa3(%rsp),%edx
mov    %edx,0x3(%rdi)
mov    %ecx,(%rdi)
mov    %al,0xf(%rsi)
mov    0x8(%rsi),%rax
xor    %rax,%r8
lea    -0x2063b1(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rcx,%r14,8),%r8
xor    %rax,%r8
mov    %r8,0x8(%rsi)
mov    0x60(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a65>
shl    $0x4,%rax
add    %rsp,%rax
add    $0xf8,%rax
lea    0xf8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a09>
nopl   0x0(%rax)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a65>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a3b>
mov    0x8(%rsi),%rdi
lea    -0x206409(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%r8
mov    %r8,%r9
and    %rdi,%r9
inc    %r9
xor    %rdi,%r9
and    %r8,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    0x0(%rbp),%rdi
lea    -0x1(%rdi),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a00>
mov    %rsi,(%rbx,%rdi,8)
mov    0x50(%rbx),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x540a>
mov    %dl,0x48(%rbx,%rdi,1)
incq   0x50(%rbx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a00>
lea    (%r15,%r10,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5425>
mov    %rsi,0xe8(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x69c6>
mov    0x8(%rsp),%rcx
mov    0x220(%rcx),%rdx
mov    %rdx,(%rax)
mov    %rbp,0x8(%rax)
mov    %rax,0x20(%rsp)
mov    %rax,0x220(%rcx)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
mov    0xe8(%rsp),%r10
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b10>
mov    %r10d,%eax
and    $0xffffffe0,%eax
lea    -0x1(%rcx),%rsi
imul   %rax,%rsi
mov    0x20(%rsp),%rdi
add    %rdi,%rsi
add    $0x40,%rsi
neg    %rax
xor    %edi,%edi
mov    0x8(%rsp),%r9
mov    0x10(%r9,%rbp,8),%r8
mov    %r8,(%rsi)
mov    %rsi,0x10(%r9,%rbp,8)
inc    %rdi
add    %rax,%rsi
cmp    %rdi,%rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5af2>
mov    %rcx,%rax
sub    %rdi,%rax
mov    %r12,0xe0(%rsp)
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
mov    0x8(%rsp),%rdx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5bd2>
lea    -0x4(%rax),%rcx
shr    $0x5,%r10d
imul   %r10,%rcx
shl    $0x5,%rcx
add    $0x40,%rcx
mov    %r10d,%r12d
shl    $0x7,%r12d
neg    %r12
lea    -0x3(%rax),%rsi
imul   %r10,%rsi
shl    $0x5,%rsi
add    $0x40,%rsi
lea    -0x2(%rax),%rdi
imul   %r10,%rdi
shl    $0x5,%rdi
add    $0x40,%rdi
lea    -0x1(%rax),%r8
imul   %r10,%r8
shl    $0x5,%r8
add    $0x40,%r8
mov    0x20(%rsp),%r9
lea    (%r9,%r8,1),%r10
mov    0x10(%rdx,%rbp,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x10(%rdx,%rbp,8)
lea    (%r9,%rdi,1),%r10
mov    0x10(%rdx,%rbp,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%rdx,%rbp,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%rdx,%rbp,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%rdx,%rbp,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%rdx,%rbp,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%rdx,%rbp,8)
add    %r12,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b7d>
mov    0x20(%rsp),%rdi
add    $0x40,%rdi
mov    0xe0(%rsp),%r12
mov    %rdi,0x20(%rsp)
xor    %esi,%esi
mov    %r12,%rdx
call   *0x16c91d(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x50(%rsp),%rcx
test   %rcx,%rcx
mov    0x40(%rsp),%rsi
mov    0x20(%rsp),%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c22>
lea    0x0(,%rcx,8),%rdx
mov    %rbp,%rdi
call   *0x16c808(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x40(%rsp),%rsi
mov    0x50(%rsp),%rcx
lea    0x0(,%rcx,8),%r12
add    %rbp,%r12
mov    0x48(%rsp),%rax
mov    %rax,0x0(%rbp,%rcx,8)
mov    0x58(%rsp),%r8
lea    0x1(%r8),%rdx
cmp    %rcx,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c5e>
lea    (%rsi,%rcx,8),%rsi
lea    0x8(%r12),%rdi
sub    %ecx,%edx
shl    $0x3,%edx
call   *0x16c7c7(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x58(%rsp),%r8
mov    0x18(%rsp),%rax
mov    (%rax),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a4c>
mov    0x70(%rsp),%rax
add    $0x18,%eax
and    $0xffffffe0,%eax
cmp    $0x1,%r8b
mov    $0x10,%ecx
cmovne %rax,%rcx
lea    -0x204555(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rax,1),%eax
cmp    $0xff,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5cc7>
mov    0x8(%rsp),%rdx
sub    %rcx,0x200(%rdx)
decq   0x208(%rdx)
cmp    $0x3e,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6bef>
mov    %rdx,%rcx
mov    0x10(%rcx,%rax,8),%rcx
mov    %rcx,(%rdi)
mov    %rdi,0x10(%rdx,%rax,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ce2>
mov    0x8(%rsp),%rax
lock sub %rcx,0x200(%rax)
lock decq 0x208(%rax)
call   *0x16c756(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x98(%rsp),%rsi
mov    0x28(%rsp),%rdx
mov    0x50(%rsp),%rdi
mov    (%rsi),%eax
mov    0x3(%rsi),%ecx
mov    %ecx,0x83(%rsp)
mov    %eax,0x80(%rsp)
cmp    %rdi,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d44>
mov    %rdi,%rdx
imul   %r14,%rdx
lea    0x1(%rdi),%r8
imul   %r14,%r8
lea    0x163376(%rip),%r9        # ADDR <anon.ADDR.2.llvm.ADDR+0x958>
lea    0x80(%rsp),%rdi
mov    $0x7,%esi
mov    %r15,%rcx
call   ADDR <<[u8]>::copy_within::<core::ops::range::Range<usize>>>
mov    0x50(%rsp),%rdi
mov    0x98(%rsp),%rsi
mov    0x10(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63d2>
lea    -0x21174f(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x1889>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %bl,0x80(%rsp,%rdi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63d2>
mov    %r8,%r13
mov    0x40(%rsp),%r15
sub    %r15,%r13
mov    0x38(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64c8>
lea    (%r12,%r15,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r12,%r15,8),%rdi
add    $0x8,%rdi
lea    0x0(,%r13,8),%rdx
mov    0x16c74a(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    %r11,0x30(%rsp)
mov    %r8,0x70(%rsp)
call   *%r12
mov    %r12,%rcx
lea    (%r15,%r15,4),%r12
mov    0x8(%rsp),%rax
lea    (%rax,%r12,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r12,1),%rdi
add    $0x5,%rdi
lea    0x0(,%r13,4),%rdx
add    %r13,%rdx
call   *%rcx
mov    %r12,%rsi
mov    0x8(%rsp),%rdi
mov    0x70(%rsp),%r8
mov    0x28(%rsp),%rdx
mov    0x30(%rsp),%r11
mov    0x10(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64cc>
mov    %r8,%r13
mov    0x40(%rsp),%r15
sub    %r15,%r13
mov    0x38(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64ea>
lea    (%r12,%r15,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r12,%r15,8),%rdi
add    $0x8,%rdi
lea    0x0(,%r13,8),%rdx
mov    0x16c6bc(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    %r11,0x30(%rsp)
mov    %r8,0x70(%rsp)
call   *%r12
mov    %r12,%rcx
lea    (%r15,%r15,2),%r12
mov    0x8(%rsp),%rax
lea    (%rax,%r12,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r12,1),%rdi
add    $0x3,%rdi
lea    0x0(,%r13,2),%rdx
add    %r13,%rdx
call   *%rcx
mov    %r12,%rsi
mov    0x8(%rsp),%rdi
mov    0x70(%rsp),%r8
mov    0x28(%rsp),%rdx
mov    0x30(%rsp),%r11
mov    0x10(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64ee>
mov    %r8,%r15
mov    0x40(%rsp),%r13
sub    %r13,%r15
mov    0x38(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x650c>
lea    (%r12,%r13,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r12,%r13,8),%rdi
add    $0x8,%rdi
lea    0x0(,%r15,8),%rdx
mov    0x16c62e(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    %r11,0x30(%rsp)
mov    %r8,0x70(%rsp)
call   *%r12
mov    %r12,%rcx
lea    0x0(,%r13,4),%r12
mov    0x8(%rsp),%rax
lea    (%rax,%r13,4),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r13,4),%rdi
add    $0x4,%rdi
shl    $0x2,%r15
mov    %r15,%rdx
call   *%rcx
mov    %r12,%rsi
mov    0x8(%rsp),%rdi
mov    0x70(%rsp),%r8
mov    0x28(%rsp),%rdx
mov    0x30(%rsp),%r11
mov    0x10(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6514>
mov    %r8,%r15
mov    0x40(%rsp),%r13
sub    %r13,%r15
mov    0x38(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6527>
lea    (%r12,%r13,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r12,%r13,8),%rdi
add    $0x8,%rdi
lea    0x0(,%r15,8),%rdx
mov    0x16c5a0(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    %r11,0x30(%rsp)
mov    %r8,0x70(%rsp)
call   *%r12
mov    %r12,%rcx
lea    0x0(,%r13,2),%r12
mov    0x8(%rsp),%rax
lea    (%rax,%r13,2),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r13,2),%rdi
add    $0x2,%rdi
add    %r15,%r15
mov    %r15,%rdx
call   *%rcx
mov    %r12,%rsi
mov    0x8(%rsp),%rdi
mov    0x70(%rsp),%r8
mov    0x28(%rsp),%rdx
mov    0x30(%rsp),%r11
mov    0x10(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x652f>
mov    %r8,%r13
mov    0x40(%rsp),%r15
sub    %r15,%r13
mov    0x38(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6543>
lea    (%r12,%r15,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r12,%r15,8),%rdi
add    $0x8,%rdi
lea    0x0(,%r13,8),%rdx
mov    0x16c513(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    %r11,0x30(%rsp)
mov    %r8,0x70(%rsp)
call   *%r12
mov    %r12,%rcx
lea    (%r15,%r15,1),%rax
lea    (%rax,%rax,2),%r12
mov    0x8(%rsp),%rax
lea    (%rax,%r12,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r12,1),%rdi
add    $0x6,%rdi
add    %r13,%r13
lea    0x0(,%r13,2),%rdx
add    %r13,%rdx
call   *%rcx
mov    %r12,%rsi
mov    0x8(%rsp),%rdi
mov    0x70(%rsp),%r8
mov    0x28(%rsp),%rdx
mov    0x30(%rsp),%r11
mov    0x10(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x654b>
mov    %r8,%rbp
mov    0x40(%rsp),%r15
sub    %r15,%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6567>
lea    (%r12,%r15,8),%rsi
lea    0x0(,%r15,8),%r13
mov    %rdi,0x8(%rsp)
lea    (%r12,%r15,8),%rdi
add    $0x8,%rdi
lea    0x0(,%rbp,8),%r12
mov    %r12,%rdx
mov    %r11,0x30(%rsp)
mov    %r8,0x70(%rsp)
call   *0x16c46f(%rip)        # ADDR <memmove@GLIBC_2.2.5>
sub    %r15,%r13
mov    0x8(%rsp),%rax
lea    (%rax,%r13,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r13,1),%rdi
add    $0x7,%rdi
sub    %rbp,%r12
mov    %r12,%rdx
call   *0x16c44a(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x8(%rsp),%rdi
mov    0x70(%rsp),%r8
mov    0x28(%rsp),%rdx
mov    0x30(%rsp),%r11
mov    0x10(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6572>
mov    %r15d,0x68(%rsp)
shr    $0x20,%r15
mov    %r15b,0x6c(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x614d>
mov    %r15w,0x68(%rsp)
shr    $0x10,%r15d
mov    %r15b,0x6a(%rsp)
mov    %bp,0x6b(%rsp)
shr    $0x10,%ebp
mov    %bpl,0x6d(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x614d>
mov    %r15d,0x68(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x614d>
xor    %r15d,%r15d
mov    0x1b0(%rsp),%r11
mov    0x60(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x614d>
mov    %r15w,0x68(%rsp)
mov    %bp,0x6a(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x614d>
mov    %r15d,0x68(%rsp)
shr    $0x20,%r15
mov    %r15w,0x6c(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x614d>
mov    %r15d,0x68(%rsp)
mov    %r15,%rcx
shr    $0x20,%rcx
mov    %cx,0x6c(%rsp)
shr    $0x30,%r15
mov    0x18(%rsp),%rsi
mov    %rax,(%rsi)
movzwl 0x6c(%rsp),%ecx
mov    %cx,0x4(%rdi)
mov    0x68(%rsp),%ecx
mov    %ecx,(%rdi)
mov    %r15b,0xe(%rsi)
lea    0x1(%r13),%ecx
mov    %cl,0xf(%rsi)
test   %rdx,%rdx
mov    0x38(%rsp),%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6205>
shl    $0x4,%rdx
lea    (%rsp,%rdx,1),%rcx
add    $0xf8,%rcx
lea    0xf8(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61a9>
cs nopw 0x0(%rax,%rax,1)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6205>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61db>
mov    0x8(%rdi),%r8
lea    -0x206ba9(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%r9
mov    %r9,%r10
and    %r8,%r10
inc    %r10
xor    %r8,%r10
and    %r9,%r10
xor    %r8,%r10
mov    %r10,0x8(%rdi)
mov    (%r14),%r8
lea    -0x1(%r8),%r9
cmp    $0x6,%r9
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61a0>
mov    %rdi,(%r11,%r8,8)
mov    0x50(%r11),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x540a>
mov    %sil,0x48(%r11,%rdi,1)
incq   0x50(%r11)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61a0>
lea    (%rax,%rbx,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5425>
lea    (%r11,%r11,4),%rax
mov    %ebx,0x78(%rsp,%rax,1)
shr    $0x20,%rbx
mov    %bl,0x7c(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6273>
lea    (%r11,%r11,2),%rax
mov    %bx,0x78(%rsp,%rax,1)
shr    $0x10,%ebx
mov    %bl,0x7a(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6273>
mov    %ebx,0x78(%rsp,%r11,4)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6273>
mov    %bx,0x78(%rsp,%r11,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6273>
lea    (%r11,%r11,2),%rax
mov    %ebx,0x78(%rsp,%rax,2)
shr    $0x20,%rbx
mov    %bx,0x7c(%rsp,%rax,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6273>
lea    (%rsp,%r11,8),%rax
add    $0x78,%rax
sub    %r11,%rax
mov    %ebx,(%rax)
mov    %rbx,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%rbx
mov    %bl,0x6(%rax)
imul   %r9d,%r14d
cmp    $0x10,%r14d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a33>
mov    0x78(%rsp),%eax
mov    0x7b(%rsp),%ecx
mov    %ecx,0x3(%r12)
mov    %eax,(%r12)
and    $0x70,%r13b
dec    %r9b
or     %r13b,%r9b
mov    0x18(%rsp),%rax
mov    %r9b,0xf(%rax)
mov    0x60(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x633d>
shl    $0x4,%rax
add    %rsp,%rax
add    $0xf8,%rax
lea    0xf8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62d9>
nopw   0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x633d>
mov    (%rcx),%r11
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x630b>
mov    0x8(%r11),%rdi
lea    -0x206cd9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%r8
mov    %r8,%r9
and    %rdi,%r9
inc    %r9
xor    %rdi,%r9
and    %r8,%r9
xor    %rdi,%r9
mov    %r9,0x8(%r11)
mov    (%r10),%rdi
lea    -0x1(%rdi),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62d0>
mov    0x1b0(%rsp),%r8
mov    %r11,(%r8,%rdi,8)
mov    0x50(%r8),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x540a>
mov    %dl,0x48(%r8,%rdi,1)
incq   0x50(%r8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62d0>
mov    0x90(%rsp),%rax
movq   $0x0,(%rax)
mov    %rsi,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57fa>
lea    (%rdi,%rdi,4),%rax
mov    %ebx,0x80(%rsp,%rax,1)
shr    $0x20,%rbx
mov    %bl,0x84(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63d2>
lea    (%rdi,%rdi,2),%rax
mov    %bx,0x80(%rsp,%rax,1)
shr    $0x10,%ebx
mov    %bl,0x82(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63d2>
mov    %ebx,0x80(%rsp,%rdi,4)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63d2>
mov    %bx,0x80(%rsp,%rdi,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63d2>
lea    (%rdi,%rdi,2),%rax
mov    %ebx,0x80(%rsp,%rax,2)
shr    $0x20,%rbx
mov    %bx,0x84(%rsp,%rax,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63d2>
lea    (%rsp,%rdi,8),%rax
add    $0x80,%rax
sub    %rdi,%rax
mov    %ebx,(%rax)
mov    %rbx,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%rbx
mov    %bl,0x6(%rax)
mov    0x30(%rsp),%rdi
imul   %edi,%r14d
cmp    $0x10,%r14d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6a65>
mov    0x18(%rsp),%rdx
mov    0x20(%rsp),%rax
mov    %rax,(%rdx)
mov    0x80(%rsp),%eax
mov    0x83(%rsp),%ecx
mov    %ecx,0x3(%rsi)
mov    %eax,(%rsi)
and    $0x70,%r13b
dec    %dil
or     %r13b,%dil
mov    %dil,0xf(%rdx)
mov    0x60(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64b0>
shl    $0x4,%rax
add    %rsp,%rax
add    $0xf8,%rax
lea    0xf8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6451>
mov    0x1b0(%rsp),%rsi
mov    %dl,0x48(%rsi,%rdi,1)
incq   0x50(%rsi)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64b0>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6483>
mov    0x8(%rsi),%rdi
lea    -0x206e51(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%r8
mov    %r8,%r9
and    %rdi,%r9
inc    %r9
xor    %rdi,%r9
and    %r8,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    0x38(%rsp),%rdi
mov    (%rdi),%rdi
lea    -0x1(%rdi),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6448>
mov    0x1b0(%rsp),%r8
mov    %rsi,(%r8,%rdi,8)
mov    0x50(%r8),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6438>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x540a>
mov    0x90(%rsp),%rax
movq   $0x0,(%rax)
mov    %r12,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57fa>
lea    (%r15,%r15,4),%rsi
mov    0x18(%rsp),%rcx
mov    0x48(%rsp),%rax
mov    %rax,(%r12,%r15,8)
mov    %edx,(%rdi,%rsi,1)
shr    $0x20,%rdx
mov    %dl,0x4(%rdi,%rsi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x659f>
lea    (%r15,%r15,2),%rsi
mov    0x18(%rsp),%rcx
mov    0x48(%rsp),%rax
mov    %rax,(%r12,%r15,8)
mov    %dx,(%rdi,%rsi,1)
shr    $0x10,%edx
mov    %dl,0x2(%rdi,%rsi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x659f>
lea    0x0(,%r13,4),%rsi
mov    0x18(%rsp),%rcx
mov    0x48(%rsp),%rax
mov    %rax,(%r12,%r13,8)
mov    %edx,(%rdi,%rsi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x659f>
lea    0x0(,%r13,2),%rsi
mov    0x18(%rsp),%rcx
mov    0x48(%rsp),%rax
mov    %rax,(%r12,%r13,8)
mov    %dx,(%rdi,%rsi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x659f>
lea    (%r15,%r15,1),%rax
lea    (%rax,%rax,2),%rsi
mov    0x18(%rsp),%rcx
mov    0x48(%rsp),%rax
mov    %rax,(%r12,%r15,8)
mov    %edx,(%rdi,%rsi,1)
shr    $0x20,%rdx
mov    %dx,0x4(%rdi,%rsi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x659f>
lea    0x0(,%r15,8),%r13
sub    %r15,%r13
mov    0x38(%rsp),%rbp
mov    0x18(%rsp),%rcx
mov    0x48(%rsp),%rax
mov    %rax,(%r12,%r15,8)
mov    %edx,(%rdi,%r13,1)
mov    %rdx,%rax
shr    $0x20,%rax
mov    %ax,0x4(%rdi,%r13,1)
shr    $0x30,%rdx
mov    %dl,0x6(%rdi,%r13,1)
mov    0x50(%rsp),%rdx
mov    (%rdx),%rax
xor    %rax,%r8
and    %r11,%r8
xor    %rax,%r8
mov    %r8,(%rdx)
cmp    $0x1,%r14d
mov    0x1b0(%rsp),%r10
mov    0xe0(%rsp),%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6615>
cmpb   $0x1,0x58(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6615>
shr    $0x8,%rbx
mov    %rbx,0x40(%r10)
movq   $0x0,0x58(%r10)
mov    %r12,0x60(%r10)
mov    %ax,0x70(%r10)
mov    %rcx,(%r10)
movb   $0x1,0x48(%r10)
movq   $0x1,0x50(%r10)
movq   $0x0,0x68(%r10)
mov    $0x1,%edx
mov    0x60(%rsp),%r9
mov    0x40(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6832>
mov    0x0(%rbp),%rax
test   %rax,%rax
mov    0x60(%rsp),%r9
mov    0x40(%rsp),%r11
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6830>
mov    0x68(%r10),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6810>
movq   $0x0,0x68(%r10)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6810>
movzbl 0x49(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x667f>
mov    0x8(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20704d(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6810>
movzbl 0x4a(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66bd>
mov    0x10(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20708b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6810>
movzbl 0x4b(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66fb>
mov    0x18(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2070c9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6810>
movzbl 0x4c(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6739>
mov    0x20(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x207107(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6810>
movzbl 0x4d(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6777>
mov    0x28(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x207145(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6810>
movzbl 0x4e(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x67b5>
mov    0x30(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x207183(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6810>
movzbl 0x4f(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x67f7>
mov    0x1b0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2071c5(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
mov    0x38(%rsp),%rbp
mov    0x60(%rsp),%r9
mov    0x40(%rsp),%r11
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b9d>
movq   $0xffffffffffffffff,0x40(%r10)
movw   $0x0,0x70(%r10)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
xor    %edx,%edx
test   %r9,%r9
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x68cc>
shl    $0x4,%r9
lea    (%rsp,%r9,1),%rax
add    $0xf8,%rax
lea    0xf8(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6879>
data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %sil,0x48(%r10,%rdi,1)
mov    0x50(%r10),%rdx
inc    %rdx
mov    %rdx,0x50(%r10)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x68cc>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x68af>
mov    0x8(%rdi),%rdx
lea    -0x207279(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rsi,8),%r8
mov    %r8,%r9
and    %rdx,%r9
inc    %r9
xor    %rdx,%r9
and    %r8,%r9
xor    %rdx,%r9
mov    %r9,0x8(%rdi)
mov    0x0(%rbp),%rdx
lea    -0x1(%rdx),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6870>
mov    %rdi,(%r10,%rdx,8)
mov    0x50(%r10),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6860>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x540a>
mov    0x10(%rsp),%rax
lea    (%rax,%r11,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5425>
lea    -0x207283(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8db>
lea    0x162768(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x910>
mov    $0xa1,%esi
call   *0x16bb8d(%rip)        # ADDR <_DYNAMIC+0x278>
lea    0x16297e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xb38>
mov    $0x7,%esi
call   *0x16bb0b(%rip)        # ADDR <_DYNAMIC+0x208>
lea    -0x207221(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc968>
lea    0x162755(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x928>
mov    $0x13,%esi
call   *0x16bcba(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x22a96d(%rip),%rdi        # ADDR <anon.ADDR.91.llvm.ADDR+0x260>
lea    0x1626ac(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x898>
mov    $0x8,%esi
call   *0x16bca1(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x22a94e(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
lea    0x162843(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa48>
mov    $0x8,%esi
call   *0x16bc88(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x1627b9(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x9d0>
mov    0x40(%rsp),%rdi
call   *0x16bb46(%rip)        # ADDR <_DYNAMIC+0x2a0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c5c>
lea    0x162802(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa30>
mov    $0x8,%esi
lea    -0x22a98a(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x698f>
lea    0x1627d5(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa18>
mov    $0x11,%esi
lea    -0x2072b5(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc957>
call   *0x16bc43(%rip)        # ADDR <_DYNAMIC+0x3d0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c5c>
lea    -0x207349(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8d5>
lea    0x162690(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8f8>
mov    $0x6,%esi
call   *0x16bc25(%rip)        # ADDR <_DYNAMIC+0x3d0>
call   *0x16bd3f(%rip)        # ADDR <_DYNAMIC+0x4f0>
test   %al,%al
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5795>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57b4>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0x16bab2(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x160fa3(%rip),%rcx        # ADDR <anon.ADDR.892.llvm.ADDR>
mov    %rdx,%rsi
mov    $0x7,%edx
mov    %r14,%rdi
call   *0x16ba6a(%rip)        # ADDR <_DYNAMIC+0x250>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c5c>
lea    -0x20733a(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc93d>
lea    0x16267f(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x940>
mov    $0xb,%esi
call   *0x16bbcc(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x16212d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
call   *0x16b9f2(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x16211b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
mov    %rax,%rdi
call   *0x16b9dd(%rip)        # ADDR <_DYNAMIC+0x208>
lea    -0x20738c(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x1626b7(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x9b8>
mov    $0x12,%esi
call   *0x16bb8c(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x207393(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc93d>
lea    0x16266e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x988>
mov    $0xb,%esi
call   *0x16bb73(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x2073be(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x16263d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x970>
mov    $0x12,%esi
call   *0x16bb5a(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x16211b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x460>
mov    $0x8,%esi
call   *0x16b980(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162589(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8e0>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16b96b(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x16255c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8c8>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16b956(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0x16b9be(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c5c>
lea    0x162472(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x808>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16b92c(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x16248d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x838>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16b917(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162490(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x850>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0x16b900(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162449(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x820>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16b8eb(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x16247c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x868>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16b8d6(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x16247f(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x880>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0x16b8bf(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x8,%esi
call   *0x16b8b4(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c5c>
lea    0x162488(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8b0>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16b89a(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161e0b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r13,%rdi
call   *0x16b885(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161dc6(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
call   *0x16b873(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x168eec(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0x16b85c(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161dcd(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r14,%rdi
call   *0x16b847(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161db8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r15,%rdi
call   *0x16b832(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161d73(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6b56>
lea    0x161d62(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
mov    %rax,%rdi
call   *0x16b80c(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x10,%edi
mov    %r15,%rsi
call   *0x16b876(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x16250f(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x9e8>
xor    %edi,%edi
xor    %esi,%esi
call   *0x16b7ed(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c5c>
mov    %r13,%rdi
lea    0x161fe9(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4d8>
mov    $0x8,%esi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c56>
lea    0x161d4b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %rbp,%rdi
call   *0x16b7c5(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1624ee(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa00>
xor    %edi,%edi
xor    %esi,%esi
call   *0x16b7b4(%rip)        # ADDR <_DYNAMIC+0x208>
ud2
mov    $0x10,%edi
mov    %rbp,%rsi
call   *0x16b81c(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %rax,%rbx
cmpq   $0x0,0xa0(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c8f>
mov    0xa8(%rsp),%rdi
call   *0x16b7b0(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c8f>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c8c>
mov    %rax,%rbx
cmpq   $0x0,0xc8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6ca8>
mov    0xd0(%rsp),%rdi
call   *0x16b790(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
