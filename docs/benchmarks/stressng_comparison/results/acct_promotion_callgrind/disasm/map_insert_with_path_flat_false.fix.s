push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x188,%rsp
mov    %r9,0x28(%rsp)
mov    %r8,0x40(%rsp)
mov    %rcx,%rbx
mov    %rsi,0x8(%rsp)
mov    %rdi,0xa0(%rsp)
mov    0x1c0(%rsp),%r12
lea    0x50(%r12),%rbp
xor    %r14d,%r14d
mov    %rbp,0x58(%rsp)
mov    %rcx,0x90(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
mov    (%r14),%rax
mov    %rax,0x18(%rdx)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r14)
movdqu %xmm0,0x30(%r14)
movdqu %xmm0,0x20(%r14)
movdqu %xmm0,0x10(%r14)
movb   $0x8,0x5(%r14)
movl   $0x0,0xab(%rsp)
movl   $0x0,0xa8(%rsp)
mov    %r14,(%rcx)
mov    0xa8(%rsp),%eax
mov    0xab(%rsp),%ecx
mov    0x10(%rsp),%rdx
mov    %eax,0x8(%rdx)
mov    0x10(%rsp),%rax
mov    %ecx,0xb(%rax)
mov    0x10(%rsp),%rdx
movb   $0x1,0xf(%rdx)
mov    $0x8,%al
mov    %rax,0x28(%rsp)
mov    %r15,%r14
nopl   0x0(%rax,%rax,1)
movzbl 0xf(%rdx),%r13d
cmp    $0xc,%r13d
mov    %rdx,0x10(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x10f3>
lea    -0x20b2cf(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x1839>
movslq (%rcx,%r13,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ec>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ca>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ca>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x148>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2002c6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ca>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x188>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200306(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ca>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200346(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ca>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x208>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200386(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ca>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x248>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2003c6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ca>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x284>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200402(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ca>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c0>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20043e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
cmpb   $0x8,0x28(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29ba>
mov    %r14,%r15
mov    0x8(%rsp),%rdx
addq   $0x40,0x200(%rdx)
incq   0x208(%rdx)
incq   0x218(%rdx)
mov    0x18(%rdx),%r14
test   %r14,%r14
mov    0x10(%rsp),%rcx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a>
movq   $0x0,0xa8(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa8(%rsp),%rdi
call   *0x170fe5(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
mov    0xa8(%rsp),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
mov    $0x1000,%edx
mov    %r14,%rdi
xor    %esi,%esi
call   *0x170fcc(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%r14)
movq   $0x1,0x8(%r14)
mov    %r14,0x220(%rdi)
lea    0xfc0(%r14),%rax
mov    $0x41,%ecx
xchg   %ax,%ax
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
jg     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a0>
add    $0x40,%r14
mov    0x10(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x51>
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63fd>
mov    (%rdx),%rax
mov    %r14,%rcx
shl    $0x4,%rcx
mov    %rdx,0x100(%rsp,%rcx,1)
mov    0x28(%rsp),%rdx
mov    %dl,0x108(%rsp,%rcx,1)
dec    %dl
mov    %rdx,0x28(%rsp)
lea    0x0(,%rdx,8),%ecx
mov    %rbx,%rdx
shr    %cl,%rdx
movzbl %dl,%ecx
inc    %r14
shl    $0x4,%ecx
lea    (%rax,%rcx,1),%rdx
add    $0x40,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
cmpb   $0x1,0x28(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x25a9>
mov    0x28(%rsp),%rax
dec    %eax
movzbl %al,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6379>
lea    -0x2005e7(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rcx
shl    $0x8,%rcx
mov    0x10(%rsp),%rax
mov    0x8(%rax),%rax
mov    %rax,%rdx
xor    %rbx,%rdx
test   %rcx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x25a9>
mov    0x0(%rbp),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x677>
mov    0x68(%r12),%rdx
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x655>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x655>
movzbl 0x49(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d3>
mov    0x8(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x200651(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x2,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x655>
movzbl 0x4a(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x513>
mov    0x10(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x200691(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x3,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x655>
movzbl 0x4b(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x553>
mov    0x18(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x2006d1(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x4,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x655>
movzbl 0x4c(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x593>
mov    0x20(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x200711(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x5,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x655>
movzbl 0x4d(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d3>
mov    0x28(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x200751(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x6,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x655>
movzbl 0x4e(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x60f>
mov    0x30(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x20078d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x7,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x655>
movzbl 0x4f(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64b>
mov    0x38(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x2007c9(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x8,%rcx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x10(%rsp),%rcx
add    $0x8,%rcx
movzbl %al,%r8d
inc    %r8
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0xab(%rsp)
mov    %eax,0xa8(%rsp)
mov    0x28(%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x625d>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6c0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x13e4>
mov    (%rdx),%r13
movzbl 0x74(%r13),%r10d
movzbl %r10b,%edi
mov    0x28(%rsp),%rax
sub    %dil,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf79>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x638b>
lea    -0x2008ad(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x10(%rsp),%rdx
mov    0x8(%rdx),%r8
mov    %r8,%rcx
xor    %rbx,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf79>
cmp    $0x7,%r10b
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63eb>
mov    %rdx,%rcx
lea    -0x2008e7(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x960>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x939>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x939>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x7b7>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200935(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x939>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x7f7>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200975(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x939>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x837>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2009b5(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x939>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x877>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2009f5(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x939>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x8b7>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200a35(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x939>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x8f3>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200a71(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x939>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x92f>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200aad(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x10(%rsp),%rcx
lea    0x8(%rcx),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0xab(%rsp)
mov    %ecx,0xa8(%rsp)
mov    0x28(%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x625d>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x9a0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x13e4>
mov    (%rdx),%rdx
movzbl 0x5(%rdx),%esi
mov    0x28(%rsp),%rax
sub    %sil,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1030>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x638b>
movzbl %sil,%edi
lea    -0x200b8c(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x10(%rsp),%r9
mov    0x8(%r9),%r8
mov    %r8,%rcx
xor    %rbx,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1030>
cmp    $0x7,%sil
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63eb>
mov    %r9,%rcx
lea    -0x200bc6(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc3f>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc18>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc18>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xa96>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200c14(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc18>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xad6>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200c54(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc18>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xb16>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200c94(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc18>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xb56>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200cd4(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc18>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xb96>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200d14(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc18>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xbd2>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200d50(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc18>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0e>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200d8c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x10(%rsp),%rcx
lea    0x8(%rcx),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0xab(%rsp)
mov    %ecx,0xa8(%rsp)
mov    0x28(%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x625d>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc70>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x13e4>
mov    (%rdx),%r15
movzbl 0x5(%r15),%ebp
mov    0x28(%rsp),%rax
sub    %bpl,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x109b>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x638b>
movzbl %bpl,%edi
lea    -0x200e5d(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x10(%rsp),%rdx
mov    0x8(%rdx),%r8
mov    %r8,%rcx
xor    %rbx,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x109b>
cmp    $0x7,%bpl
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63eb>
mov    %rdx,%rcx
lea    -0x200e97(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    0x58(%rsp),%rbp
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf15>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xeee>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xeee>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xd6c>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200eea(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xeee>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xdac>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200f2a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xeee>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xdec>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200f6a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xeee>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xe2c>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200faa(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xeee>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xe6c>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x200fea(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xeee>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xea8>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201026(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xeee>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xee4>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201062(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x10(%rsp),%rcx
lea    0x8(%rcx),%rax
inc    %r8
mov    (%rax),%ecx
mov    0x3(%rax),%eax
mov    %eax,0xab(%rsp)
mov    %ecx,0xa8(%rsp)
mov    0x28(%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x625d>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf50>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x13e4>
mov    %r14,0x68(%rsp)
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
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1403>
mov    0x68(%rsp),%r14
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x643f>
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
mov    0x10(%rsp),%rdx
mov    %rdx,0x100(%rsp,%rcx,1)
mov    %r10b,0x108(%rsp,%rcx,1)
inc    %r14
mov    %esi,%ecx
mov    %rcx,0x28(%rsp)
mov    %rax,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
lea    -0x1(%rsi),%edi
lea    0x0(,%rdi,8),%ecx
mov    %rbx,%rax
shr    %cl,%rax
movzbl 0x4(%rdx),%ecx
test   %ecx,%ecx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x21bc>
movzbl 0x8(%rdx),%r8d
cmp    %al,%r8b
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1711>
xor    %r9d,%r9d
cmp    $0x7,%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64aa>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rcx
mov    %rcx,0x100(%rsp,%rax,1)
mov    %sil,0x108(%rsp,%rax,1)
inc    %r14
shl    $0x4,%r9d
add    %r9,%rdx
add    $0x10,%rdx
mov    %rdi,0x28(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
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
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1796>
cmp    $0x4,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1753>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cc8>
cmp    %dl,0x8(%r15)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cc8>
xor    %ecx,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cdf>
cmp    $0x7f,%r13d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6244>
lea    -0x5(%r13),%eax
cmp    $0x7,%al
mov    %r14,0x68(%rsp)
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x263c>
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x131b>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12f4>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12f4>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1172>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2012f0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12f4>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x11b2>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201330(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12f4>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x11f2>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201370(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12f4>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1232>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2013b0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12f4>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1272>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2013f0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12f4>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12ae>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20142c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12f4>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12ea>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201468(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x10(%rsp),%rdx
lea    -0x4(%r13),%eax
lea    0x8(%rdx),%rcx
mov    %rcx,0x50(%rsp)
movzbl %al,%esi
mov    0x8(%rdx),%rax
lea    -0x2014ce(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rsi,8),%rbp
mov    %rbp,%r14
and    %rax,%r14
lea    0x1(%r14),%r8
mov    0x28(%rsp),%rcx
cmp    %sil,%cl
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29ca>
sub    %sil,%cl
movzbl %cl,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6379>
lea    -0x201501(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rdi,8),%rdx
lea    0x0(,%rsi,8),%r15d
mov    %r15d,%ecx
shl    %cl,%rdx
xor    %rbx,%rax
test   %rax,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29d9>
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0xab(%rsp)
mov    %eax,0xa8(%rsp)
mov    0x28(%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
mov    0x58(%rsp),%rbp
mov    0x68(%rsp),%r14
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x625d>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x13c0>
movzbl %dl,%edx
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%r15
mov    %r15,%rsi
mov    %eax,%ecx
call   ADDR <expanse_trie::mutate::wrap_skip_level::<false>>
mov    %r15,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
movdqa 0x0(%r13),%xmm0
movdqa 0x10(%r13),%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x1,%xmm2
movdqa -0x225f10(%rip),%xmm3        # ADDR <anon.ADDR.407.llvm.ADDR+0x10>
pand   %xmm3,%xmm2
psubb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
movdqa -0x225c74(%rip),%xmm4        # ADDR <anon.ADDR.407.llvm.ADDR+0x2c0>
pand   %xmm4,%xmm2
psrlw  $0x2,%xmm1
pand   %xmm4,%xmm1
paddb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x4,%xmm2
paddb  %xmm1,%xmm2
movdqa -0x225fea(%rip),%xmm5        # ADDR <anon.ADDR.283.llvm.ADDR+0xf6>
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
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19ce>
cmp    %r10b,0x28(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2287>
cmp    $0x8,%r10b
mov    0x68(%rsp),%r14
mov    0x10(%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63eb>
lea    -0x201678(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%r8
and    0x8(%rcx),%r8
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16f0>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ce>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ce>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x154c>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2016ca(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ce>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x158c>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20170a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ce>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x15cc>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20174a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ce>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x160c>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20178a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ce>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x164c>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2017ca(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ce>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1688>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201806(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ce>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16c4>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201842(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
inc    %r8
inc    %r10b
movzbl 0x28(%rsp),%ecx
movzbl %r10b,%edx
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%r15
mov    %r15,%rsi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x13f6>
cmp    $0x1,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1a76>
cmp    %al,%r8b
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d35>
lea    0x10(%rdx),%r9
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x21ae>
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x21bc>
movzbl 0x9(%rdx),%r10d
mov    %r10b,0xa(%rdx)
mov    %r8b,0x9(%rdx)
movups 0x20(%rdx),%xmm0
movups %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x21b2>
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
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1796>
tzcnt  %esi,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cdf>
cmp    $0x7,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1db3>
mov    0x58(%rsp),%rbp
mov    0x0(%rbp),%rax
test   %rax,%rax
mov    0x1c0(%rsp),%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19b4>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1992>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1992>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1810>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20198e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1992>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1850>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2019ce(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1992>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1890>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201a0e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1992>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x18d0>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201a4e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1992>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1910>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201a8e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1992>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x194c>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201aca(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1992>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1988>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201b06(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%r15
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate::upgrade_l7_to_b::<false>>
mov    %r15,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
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
mov    %rax,0x28(%rsp)
test   %r8,%r8
mov    %r10,0x50(%rsp)
mov    %rsi,0x20(%rsp)
mov    %r8,0x38(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1e36>
mov    $0x20,%edx
cmp    $0x1,%r8d
mov    0x8(%rsp),%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1e40>
cmp    $0x10,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2224>
cmp    $0x3,%r8w
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1a62>
lea    0x4(%r8),%eax
lea    0x3(%r8),%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24cb>
mov    %r8d,%edx
shl    $0x4,%edx
add    $0x40,%edx
and    $0x1c0,%edx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1e3b>
mov    $0x1,%r9d
cmp    %al,0x9(%rdx)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x105d>
cmp    $0x2,%cl
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x171a>
mov    $0x2,%r9d
cmp    %al,0xa(%rdx)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x105d>
cmp    $0x3,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x171a>
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cae>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8c>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8c>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1b0a>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201c88(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8c>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1b4a>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201cc8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8c>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1b8a>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201d08(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8c>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1bca>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201d48(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8c>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c0a>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201d88(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8c>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c46>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201dc4(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8c>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c82>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x201e00(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%r15
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate::upgrade_l3_to_l7::<false>>
mov    %r15,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
cmp    $0x2,%al
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d70>
mov    $0x1,%ecx
cmp    %dl,0x9(%r15)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d70>
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x647e>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rdx
mov    %rdx,0x100(%rsp,%rax,1)
mov    %bpl,0x108(%rsp,%rax,1)
cmp    $0x7,%ecx
mov    0x1c0(%rsp),%r12
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6493>
inc    %r14
shl    $0x4,%ecx
lea    (%r15,%rcx,1),%rdx
add    $0x10,%rdx
mov    %edi,%eax
mov    %rax,0x28(%rsp)
mov    0x58(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
movzbl 0x9(%rdx),%r9d
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d4e>
mov    $0x2,%r8d
cmp    %al,%r9b
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x21bf>
mov    $0x1,%r8d
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x21bf>
mov    %r9b,0xa(%rdx)
movdqu 0x20(%rdx),%xmm0
movdqu %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x21bf>
cmp    $0x3,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1db3>
cmp    %dl,0xa(%r15)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1db3>
cmp    $0x7,%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x647e>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rcx
mov    %rcx,0x100(%rsp,%rax,1)
mov    %bpl,0x108(%rsp,%rax,1)
mov    $0x2,%ecx
mov    0x1c0(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d16>
mov    %rdi,%rbx
lea    0x8(%r15),%rdi
lea    0x10(%r15),%r13
movzbl %dl,%r8d
mov    $0x7,%edx
mov    %r13,%rsi
mov    %rax,%rcx
call   *0x16f815(%rip)        # ADDR <_DYNAMIC+0x5b0>
incb   0x4(%r15)
or     %r12w,0x6(%r15)
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64bf>
mov    %r14,%rcx
shl    $0x4,%rcx
mov    0x10(%rsp),%rdx
mov    %rdx,0x100(%rsp,%rcx,1)
mov    %bpl,0x108(%rsp,%rcx,1)
cmp    $0x7,%eax
mov    0x1c0(%rsp),%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64d4>
inc    %r14
shl    $0x4,%eax
add    %rax,%r13
mov    %rbx,0x28(%rsp)
mov    %r13,%rdx
mov    0x58(%rsp),%rbp
mov    0x90(%rsp),%rbx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
mov    $0x10,%edx
mov    0x8(%rsp),%rcx
lea    -0x1ffebf(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rdx,%rax,1),%ebp
cmp    $0xff,%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x25a2>
add    %rdx,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3e,%bpl
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6510>
mov    0x10(%rcx,%rbp,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1e8e>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x201b>
cmp    $0x101,%edx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x223f>
mov    %rdx,0x30(%rsp)
movq   $0x0,0xa8(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa8(%rsp),%rdi
call   *0x16f46d(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
mov    0xa8(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
mov    $0x1000,%edx
xor    %esi,%esi
mov    %rdi,0x18(%rsp)
call   *0x16f452(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x18(%rsp),%r9
mov    0x8(%rsp),%r12
mov    0x220(%r12),%rax
mov    %rax,(%r9)
mov    %rbp,0x8(%r9)
mov    %r9,0x220(%r12)
mov    $0xfc0,%ax
xor    %edx,%edx
mov    0x30(%rsp),%r10
div    %r10w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f56>
mov    %rcx,%rax
dec    %rax
mov    %rax,%rsi
imul   %r10,%rsi
lea    (%r9,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%r12,%rbp,8),%r8
mov    %r8,0x40(%r9,%rsi,1)
mov    %rdi,0x10(%r12,%rbp,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f30>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2005>
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
mov    0x18(%rsp),%r9
nopl   0x0(%rax,%rax,1)
lea    (%r9,%r8,1),%r10
mov    0x10(%r12,%rbp,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x10(%r12,%rbp,8)
lea    (%r9,%rdi,1),%r10
mov    0x10(%r12,%rbp,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%r12,%rbp,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%r12,%rbp,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%r12,%rbp,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%r12,%rbp,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%r12,%rbp,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1fb0>
mov    0x18(%rsp),%rdi
add    $0x40,%rdi
mov    0x1c0(%rsp),%r12
mov    0x30(%rsp),%rdx
xor    %esi,%esi
mov    %rdi,%rbp
call   *0x16f31a(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %rbp,%r9
mov    0x58(%rsp),%rbp
mov    0x38(%rsp),%r8
test   %r8,%r8
mov    0x50(%rsp),%rdx
mov    0x20(%rsp),%rsi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2130>
mov    %r13,0x88(%rsp)
mov    0x20(%r13,%r15,8),%rsi
mov    %rsi,0x30(%rsp)
mov    0x28(%rsp),%rbp
shl    $0x4,%ebp
mov    %r9,%rdi
mov    %rbp,%rdx
mov    %r8,%r13
mov    %r9,%r12
call   *0x16f1de(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r12,0x18(%rsp)
lea    (%r12,%rbp,1),%rdi
add    $0x10,%rdi
mov    0x30(%rsp),%r12
add    %r12,%rbp
mov    %r13,%rdx
sub    0x28(%rsp),%rdx
shl    $0x4,%rdx
mov    %rbp,%rsi
call   *0x16f1b4(%rip)        # ADDR <memcpy@GLIBC_2.14>
test   %r12,%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6288>
mov    %r12,%r10
mov    %r13,%rsi
cmp    $0x3,%r13w
mov    0x58(%rsp),%rbp
mov    0x8(%rsp),%rcx
mov    0x50(%rsp),%rdx
mov    0x1c0(%rsp),%r12
mov    %r13,%r8
mov    0x88(%rsp),%r13
mov    0x18(%rsp),%r9
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x20eb>
cmp    $0x11,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2521>
lea    0x3(%r8),%esi
and    $0x3c,%esi
shl    $0x4,%esi
lea    -0x20016d(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rsi,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2545>
sub    %rsi,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6525>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%r10)
mov    %r10,0x10(%rcx,%rdi,8)
mov    0x20(%rsp),%rsi
mov    0x28(%rsp),%rax
shl    $0x4,%eax
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r9,%rax,1)
mov    %r9,0x20(%r13,%r15,8)
mov    $0x1,%eax
mov    %r14d,%ecx
shl    %cl,%rax
inc    %r8d
mov    %r8w,0x60(%r13,%r15,2)
shr    $0x3,%r14d
and    $0x18,%r14d
or     %rax,0x0(%r13,%r14,1)
mov    0x68(%rsp),%r14
cmp    $0x7,%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64fb>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rcx
mov    %rcx,0x100(%rsp,%rax,1)
mov    %dl,0x108(%rsp,%rax,1)
inc    %r14
mov    0x28(%rsp),%rdx
shl    $0x4,%edx
add    0x20(%r13,%r15,8),%rdx
mov    %esi,%eax
mov    %rax,0x28(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
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
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6469>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rcx
mov    %rcx,0x100(%rsp,%rax,1)
add    %r8,%rdx
add    $0x10,%rdx
mov    %sil,0x108(%rsp,%rax,1)
inc    %r14
mov    %rdi,0x28(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
cmp    $0x18,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2268>
cmp    $0x10,%r8d
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24cb>
mov    $0x180,%edx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2585>
mov    0x58(%rsp),%rbp
mov    %rdx,0x30(%rsp)
mov    $0x1,%esi
mov    %rdx,%rdi
call   *0x16f0e1(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r9
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x202e>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65bd>
cmp    $0x20,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24ae>
cmp    $0x18,%r8d
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24cb>
mov    $0x200,%edx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2585>
mov    0x0(%rbp),%rax
test   %rax,%rax
mov    0x68(%rsp),%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2494>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2472>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2472>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x22f0>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20246e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2472>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2330>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2024ae(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2472>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2370>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2024ee(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2472>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x23b0>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20252e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2472>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x23f0>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20256e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2472>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x242c>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2025aa(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2472>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2468>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2025e6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%r15
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate::upgrade_b_to_u::<false>>
mov    %r15,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
cmp    $0x20,%r8d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2572>
lea    0x4(%r8),%eax
lea    0x3(%r8),%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2579>
mov    0x20(%r13,%r15,8),%r12
mov    0x28(%rsp),%rax
mov    %eax,%ebp
shl    $0x4,%ebp
lea    (%r12,%rbp,1),%rsi
lea    (%r12,%rbp,1),%rdi
add    $0x10,%rdi
mov    %r8,%rdx
sub    %rax,%rdx
shl    $0x4,%rdx
call   *0x16ee2a(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x38(%rsp),%r8
mov    0x20(%rsp),%rsi
mov    0x50(%rsp),%rdx
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12,%rbp,1)
mov    0x58(%rsp),%rbp
mov    0x1c0(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2147>
mov    $0x180,%esi
cmp    $0x19,%r8w
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2545>
mov    $0x200,%esi
cmp    $0x21,%r8w
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2545>
mov    %r8d,%esi
shl    $0x4,%esi
add    $0x30,%esi
and    $0xffffffc0,%esi
mov    %rcx,%rdi
call   *0x16f022(%rip)        # ADDR <_DYNAMIC+0x538>
mov    0x30(%rsp),%rdi
call   *0x16ed0f(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x38(%rsp),%r8
mov    0x50(%rsp),%rdx
mov    0x20(%rsp),%rsi
mov    0x18(%rsp),%r9
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2130>
mov    $0x240,%edx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2585>
mov    %r8d,%edx
shl    $0x4,%edx
add    $0x40,%edx
and    $0xffffffc0,%edx
mov    %rdx,0x30(%rsp)
mov    0x8(%rsp),%rdi
mov    %rdx,%rsi
call   *0x16efa0(%rip)        # ADDR <_DYNAMIC+0x500>
mov    0x30(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2244>
mov    0x58(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2585>
mov    %ebx,%r9d
shr    $0x5,%r9d
and    $0x7,%r9d
mov    $0x1,%eax
mov    %ebx,%ecx
shl    %cl,%eax
mov    0x10(%rsp),%rcx
mov    (%rcx),%r10
mov    (%r10,%r9,4),%r13d
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
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2a8c>
mov    0x20(%r10,%r9,8),%rax
lea    (%rax,%r8,8),%rcx
mov    (%rax,%r8,8),%rdx
mov    0x40(%rsp),%rsi
mov    %rsi,(%rax,%r8,8)
mov    0xa0(%rsp),%rax
movq   $0x1,(%rax)
mov    %rdx,0x8(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c1c>
test   %r13b,%r13b
js     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x626f>
mov    %r13d,%r14d
shr    $0x4,%r14d
test   %r14b,%r14b
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x626f>
mov    %r13d,%r10d
and    $0xf,%r10d
lea    0x1(%r10),%rsi
mov    %rsi,%r15
imul   %r14,%r15
cmp    $0xf,%r15
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x626f>
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2879>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2857>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2857>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x26d5>
mov    0x8(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x202853(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2857>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2715>
mov    0x10(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x202893(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2857>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2755>
mov    0x18(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x2028d3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2857>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2795>
mov    0x20(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x202913(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2857>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27d5>
mov    0x28(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x202953(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2857>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2811>
mov    0x30(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x20298f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2857>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x284d>
mov    0x38(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x2029cb(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
lea    0x0(,%r14,8),%ecx
mov    $0xffffffffffffffff,%rbp
shl    %cl,%rbp
not    %rbp
and    %rbp,%rbx
mov    0x10(%rsp),%rcx
lea    0x8(%rcx),%rdi
test   %r10b,%r10b
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c79>
mov    %r10,0x28(%rsp)
movzbl %r14b,%edx
mov    %rdi,0x50(%rsp)
mov    %rdx,0x20(%rsp)
mov    %rbx,%rcx
mov    %rsi,%rbp
call   ADDR <expanse_trie::leaf::locate>
mov    %rdx,%r11
mov    0x10(%rsp),%rcx
mov    (%rcx),%r8
test   $0x1,%al
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cb0>
mov    $0x7,%al
movzbl %al,%eax
div    %r14b
lea    0x4(%rbp),%rcx
cmp    %bpl,%al
mov    %rbp,0x18(%rsp)
mov    %r11,0x90(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2e4d>
mov    %rbp,%rdx
lea    0x1(%rbp),%r9
cmpb   $0x1,0x28(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2fe5>
lea    0x3(%rdx),%rax
xor    %rax,%rcx
cmp    $0x4,%ecx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2fe5>
lea    (%r8,%r11,8),%rsi
cmp    %r11,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e20>
lea    0x1(%r11),%rbp
mov    %r9,0x88(%rsp)
lea    (%r8,%r11,8),%rdi
add    $0x8,%rdi
sub    %r11d,%edx
shl    $0x3,%edx
mov    %r8,%r12
mov    %rsi,0x28(%rsp)
call   *0x16e9d3(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x40(%rsp),%rax
mov    0x90(%rsp),%rcx
mov    %rax,(%r12,%rcx,8)
mov    0x50(%rsp),%r12
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %eax,0x70(%rsp)
mov    %ecx,0x73(%rsp)
mov    0x90(%rsp),%rdx
imul   %r14,%rdx
imul   %r14,%rbp
lea    0x165585(%rip),%r9        # ADDR <anon.ADDR.2.llvm.ADDR+0x9a0>
lea    0x70(%rsp),%rdi
mov    $0x7,%esi
mov    %r15,%rcx
mov    %rbp,%r8
call   ADDR <<[u8]>::copy_within::<core::ops::range::Range<usize>>>
mov    0x28(%rsp),%rsi
mov    0x88(%rsp),%r9
mov    0x90(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e3e>
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2b05>
mov    0x28(%rsp),%r10
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2b22>
mov    %rsi,0x20(%rsp)
lea    0x0(,%rsi,8),%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29de>
mov    %rsi,0x20(%rsp)
mov    $0xffffffffffffffff,%r10
mov    %r15d,%ecx
shl    %cl,%r10
not    %r10
and    %rbx,%r10
mov    0x10(%rsp),%rax
mov    (%rax),%r11
cmp    $0x3,%r8
mov    %r11,0x18(%rsp)
mov    %r8,0x88(%rsp)
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2a72>
mov    %r8,%rax
test   %r8,%r8
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c3f>
mov    0x20(%rsp),%rax
cmp    $0x1,%rax
sete   %dl
mov    %rax,%r13
lea    0x8(%rax),%r11
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x9b(%rsp)
mov    %eax,0x98(%rsp)
mov    $0x1,%eax
mov    %rax,0xf0(%rsp)
movq   $0x0,0x78(%rsp)
movq   $0x0,0xd0(%rsp)
xor    %r15d,%r15d
mov    0x8(%rsp),%rdi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3fa3>
cmp    $0x11,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c25>
mov    %r14,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c3f>
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
imul   $0x1010101,%eax,%ecx
shr    $0x18,%ecx
test   %r13d,%r13d
mov    0x8(%rsp),%rdi
mov    %r14,0x68(%rsp)
mov    %r10,0x50(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30c1>
lea    -0x1(%r13),%eax
test   %eax,%r13d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2e40>
cmp    $0x10,%ecx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ecd>
lea    0x4(%rcx),%edx
and    $0x1c,%edx
cmp    $0x2,%rcx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ee4>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30fd>
mov    0x28(%rsp),%r10
lea    0x0(,%r10,8),%ecx
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
shl    $0x4,%r10b
mov    0x10(%rsp),%rax
mov    0x40(%rsp),%r9
mov    %r9,(%rax)
mov    %bl,0x8(%rax)
mov    %bh,0x9(%rax)
mov    %r8b,0xa(%rax)
mov    %cl,0xb(%rax)
mov    %dl,0xc(%rax)
mov    %sil,0xd(%rax)
mov    %dil,0xe(%rax)
mov    %r10b,0xf(%rax)
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c08>
shl    $0x4,%r14
lea    (%rsp,%r14,1),%rax
add    $0x100,%rax
lea    0x100(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ba9>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c08>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2bdb>
mov    0x8(%rsi),%rdi
lea    -0x202d59(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ba0>
mov    %rsi,(%r12,%rdi,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5149>
mov    %dl,0x48(%r12,%rdi,1)
incq   0x50(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ba0>
mov    0xa0(%rsp),%rax
movq   $0x0,(%rax)
mov    0x10(%rsp),%rcx
mov    %rcx,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54da>
mov    $0x18,%eax
cmp    $0x19,%r8
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c3f>
mov    $0x20,%eax
cmp    $0x21,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2a7c>
lea    (%r11,%rax,8),%rdi
mov    0x20(%rsp),%rax
dec    %eax
cmp    $0x5,%eax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d29>
lea    -0x20ddaa(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x18dd>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
movzbl (%rdi,%r14,1),%eax
mov    %r8,%rsi
cmp    %rax,%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d54>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37ad>
and    0x8(%rcx),%rbp
mov    (%rcx),%rax
cmp    %rbx,%rbp
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2dd6>
mov    0x40(%rsp),%rdx
mov    %rdx,(%rcx)
mov    %rcx,%rdx
mov    0xa0(%rsp),%rcx
movq   $0x1,(%rcx)
mov    %rax,0x8(%rcx)
mov    %rdx,0x10(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54da>
lea    (%r8,%r11,8),%rax
mov    (%r8,%r11,8),%rcx
mov    0x40(%rsp),%rdx
mov    %rdx,(%r8,%r11,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d6a>
lea    (%r14,%r14,4),%rax
mov    (%rdi,%rax,1),%ecx
movzbl 0x4(%rdi,%rax,1),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d15>
lea    (%r14,%r14,2),%rax
movzwl (%rdi,%rax,1),%ecx
movzbl 0x2(%rdi,%rax,1),%eax
shl    $0x10,%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d19>
mov    (%rdi,%r14,4),%eax
mov    %r8,%rsi
cmp    %rax,%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d54>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37ad>
movzwl (%rdi,%r14,2),%eax
mov    %r8,%rsi
cmp    %rax,%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d54>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37ad>
lea    (%r14,%r14,2),%rax
mov    (%rdi,%rax,2),%ecx
movzwl 0x4(%rdi,%rax,2),%eax
shl    $0x20,%rax
or     %rcx,%rax
mov    %r8,%rsi
cmp    %rax,%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d54>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37ad>
lea    (%rdi,%r14,8),%rax
sub    %r14,%rax
mov    (%rax),%ecx
movzwl 0x4(%rax),%edx
movzbl 0x6(%rax),%eax
shl    $0x20,%rdx
or     %rcx,%rdx
shl    $0x30,%rax
or     %rdx,%rax
mov    %r8,%rsi
cmp    %rax,%r10
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37ad>
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d86>
mov    %r14,%rsi
lea    (%r11,%rsi,8),%rax
mov    (%r11,%rsi,8),%rcx
mov    0x40(%rsp),%rdx
mov    %rdx,(%r11,%rsi,8)
mov    0xa0(%rsp),%rdx
movq   $0x1,(%rdx)
mov    %rcx,0x8(%rdx)
mov    %rax,0x10(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54da>
mov    0x20(%rsp),%rax
dec    %eax
cmp    $0x5,%eax
mov    %rbp,0x48(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3745>
lea    -0x20deda(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x18f5>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    %rdi,%rbp
mov    %r8,%rsi
mov    %r10,%r12
mov    %r10,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<1>>
lea    0x1(%r14),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37a0>
movzbl 0x0(%rbp,%rsi,1),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x370d>
setbe  %cl
mov    %rax,%r12
mov    0x40(%rsp),%rdx
cmovbe %rdx,%r12
cmovbe %rax,%rdx
mov    %rbp,%r15
cmova  %rbx,%r15
cmovb  %rbx,%rbp
movzbl %cl,%ebx
cmp    $0x40,%r13b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2eef>
mov    %rdx,0x40(%rsp)
mov    0x8(%rsp),%rdx
addq   $0x10,0x200(%rdx)
incq   0x208(%rdx)
incq   0x218(%rdx)
mov    0x80(%rdx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e6d>
mov    (%rax),%rcx
mov    %rcx,0x80(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f21>
mov    $0x2,%edx
mov    %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3080>
and    $0xfffffffc,%ecx
cmpb   $0xf,0x28(%rsp)
mov    $0x18,%eax
cmovne %rcx,%rax
lea    0x8(%r14),%rdx
imul   %rax,%rdx
lea    0xc(%rdx),%rsi
mov    %esi,%r13d
and    $0xfffffff0,%r13d
lea    -0x200ef0(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rdx,%rax,1),%ebp
cmp    $0xff,%rbp
mov    %r8,0x38(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66c4>
mov    0x8(%rsp),%rcx
add    %r13,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3d,%bpl
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6510>
mov    0x10(%rcx,%rbp,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x343f>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3598>
cmp    $0x18,%ecx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x305f>
mov    $0x18,%edx
cmp    $0x10,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3070>
lea    0x3(%rcx),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3080>
mov    %r15,0xa8(%rsp)
mov    %rdx,0xb0(%rsp)
mov    %rbp,0xb8(%rsp)
mov    %r12,0xc0(%rsp)
movzbl %r14b,%edx
lea    0xa8(%rsp),%rcx
mov    $0x2,%r8d
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%rsi
call   ADDR <expanse_trie::mutate_map::build_map_leaf::<false>>
mov    0x68(%rsp),%rax
test   %rax,%rax
mov    0x1c0(%rsp),%r10
mov    0x58(%rsp),%r11
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2fc2>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x100,%rax
lea    0x100(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f70>
mov    %dl,0x48(%r10,%rdi,1)
incq   0x50(%r10)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2fc2>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2fa2>
mov    0x8(%rsi),%rdi
lea    -0x203120(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f67>
mov    %rsi,(%r10,%rdi,8)
mov    0x50(%r10),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f5e>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5149>
shl    $0x3,%ebx
mov    0x10(%rsp),%rax
add    (%rax),%rbx
mov    0xa0(%rsp),%rax
movq   $0x0,(%rax)
mov    %rbx,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54da>
lea    0x0(,%rdx,8),%esi
lea    0x20(%rsi),%r10
mov    %r10d,%edx
and    $0xffffffe0,%edx
lea    -0x201075(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rdx,%rax,1),%ebp
cmp    $0xff,%rbp
mov    %r8,0x38(%rsp)
mov    %r9,0x88(%rsp)
mov    %rsi,0x78(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6738>
mov    0x8(%rsp),%rcx
add    %rdx,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3d,%bpl
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6510>
mov    0x10(%rcx,%rbp,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x464c>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47b7>
xor    %eax,%eax
cmp    $0xffffffff,%r13d
sete   %al
lea    0x20(,%rax,4),%rdx
xor    %eax,%eax
cmp    $0x19,%ecx
setae  %al
lea    0x18(,%rax,8),%rax
cmp    %eax,%edx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30c1>
mov    0x20(%r10,%r9,8),%r14
lea    (%r14,%r8,8),%rsi
lea    (%r14,%r8,8),%rdi
add    $0x8,%rdi
sub    %r8,%rcx
shl    $0x3,%rcx
mov    %rcx,%rdx
mov    %r8,%r13
mov    %r9,%r15
call   *0x16e275(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x50(%rsp),%r9
mov    %r15,%r8
mov    0x40(%rsp),%rax
mov    %rax,(%r14,%r13,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3319>
lea    -0x1(%r13),%eax
test   %eax,%r13d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30d7>
cmp    $0xf,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30dd>
lea    0x4(%rcx),%edx
and    $0x1c,%edx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30fd>
lea    0x1(%rcx),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30fd>
mov    $0xc0,%ebp
mov    $0xc0,%edx
cmp    $0x18,%ecx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3108>
xor    %eax,%eax
cmp    $0xffffffff,%r13d
sete   %al
lea    0x20(,%rax,4),%rdx
shl    $0x3,%edx
lea    0x8(%rdx),%rbp
and    $0xfffffffffffffff0,%rbp
lea    -0x201187(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rdx,%rax,1),%r14d
cmp    $0xff,%r14
mov    %r8,0x20(%rsp)
mov    %r9,0x38(%rsp)
mov    %rcx,0x18(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6647>
add    %rbp,0x200(%rdi)
incq   0x208(%rdi)
incq   0x218(%rdi)
cmp    $0x3e,%r14b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x654c>
mov    0x10(%rdi,%r14,8),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3166>
mov    (%r15),%rax
mov    %rax,0x10(%rdi,%r14,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3200>
cmp    $0x101,%edx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6656>
mov    %rdx,%r12
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
mov    %rax,%r15
mov    0x8(%rsp),%rcx
mov    0x220(%rcx),%rax
mov    %rax,(%r15)
mov    %r14,0x8(%r15)
mov    %r15,0x220(%rcx)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %bp
movzwl %ax,%eax
cs nopw 0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %rbp,%rcx
lea    (%r15,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x8(%rsp),%rsi
mov    0x10(%rsi,%r14,8),%rsi
mov    %rsi,0x40(%r15,%rcx,1)
mov    0x8(%rsp),%rcx
mov    %rdx,0x10(%rcx,%r14,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x31c0>
add    $0x40,%r15
mov    %r12,%rdx
mov    0x1c0(%rsp),%r12
mov    %r15,%rdi
xor    %esi,%esi
call   *0x16e135(%rip)        # ADDR <memset@GLIBC_2.2.5>
test   %r13d,%r13d
mov    0x20(%rsp),%r13
mov    0x38(%rsp),%r8
mov    0x50(%rsp),%r9
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3306>
mov    0x20(%r9,%r8,8),%rbp
lea    0x0(,%r13,8),%r14d
mov    %r15,%rdi
mov    %rbp,%rsi
mov    %r14,%rdx
call   *0x16e011(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    (%r15,%r14,1),%rdi
add    $0x8,%rdi
add    %rbp,%r14
mov    %rbp,%rax
mov    0x18(%rsp),%rbp
mov    %rbp,%rdx
sub    %r13,%rdx
shl    $0x3,%rdx
mov    %r14,%rsi
mov    %rax,%r14
call   *0x16dfe8(%rip)        # ADDR <memcpy@GLIBC_2.14>
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6307>
cmp    $0x3,%ebp
mov    0x8(%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x328c>
mov    0x20(%rsp),%r13
mov    0x38(%rsp),%r8
mov    0x50(%rsp),%r9
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x32b8>
cmp    $0x11,%ebp
mov    0x20(%rsp),%r13
mov    0x38(%rsp),%r8
mov    0x50(%rsp),%r9
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x32a8>
add    $0x3,%ebp
and    $0x3c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x32b8>
xor    %eax,%eax
cmp    $0x19,%ebp
setae  %al
lea    0x18(,%rax,8),%rbp
lea    0x0(,%rbp,8),%eax
lea    0x8(,%rbp,8),%esi
and    $0xfffffff0,%esi
lea    -0x201348(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rax,%rdx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x668e>
sub    %rsi,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3d,%dil
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6525>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%r14)
mov    %r14,0x10(%rcx,%rdi,8)
mov    0x40(%rsp),%rax
mov    %rax,(%r15,%r13,8)
mov    %r15,0x20(%r9,%r8,8)
mov    0x58(%rsp),%rbp
mov    0x68(%rsp),%rdx
mov    $0x1,%eax
mov    %ebx,%ecx
shl    %cl,%rax
mov    %ebx,%ecx
shr    $0x3,%ecx
and    $0x18,%ecx
or     %rax,(%r9,%rcx,1)
mov    0x10(%rsp),%rsi
mov    0x8(%rsi),%rax
lea    0x1(%rax),%ecx
mov    %cl,0x8(%rsi)
cmpb   $0x1,0x28(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3388>
mov    %rsi,%rcx
shr    $0x8,%rbx
mov    %rbx,0x40(%r12)
mov    (%rcx),%rcx
mov    %rcx,0x58(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x60(%r12)
movzbl %al,%eax
add    $0x2,%eax
mov    %ax,0x70(%r12)
mov    %rsi,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
shl    $0x3,%r13d
add    0x20(%r9,%r8,8),%r13
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3427>
shl    $0x4,%rdx
lea    (%rsp,%rdx,1),%rax
add    $0x100,%rax
lea    0x100(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33d3>
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %dl,0x48(%r12,%rdi,1)
incq   0x50(%r12)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3427>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3405>
mov    0x8(%rsi),%rdi
lea    -0x203583(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33ca>
mov    %rsi,(%r12,%rdi,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33c0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5149>
mov    0xa0(%rsp),%rax
movq   $0x0,(%rax)
mov    %r13,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54da>
cmp    $0x101,%edx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66d8>
mov    %rsi,0x88(%rsp)
mov    %rdx,0x30(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
mov    %rax,%r9
mov    0x8(%rsp),%r15
mov    0x220(%r15),%rax
mov    %rax,(%r9)
mov    %rbp,0x8(%r9)
mov    %r9,0x220(%r15)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r13w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x34cf>
mov    %rcx,%rax
dec    %rax
mov    %rax,%rsi
imul   %r13,%rsi
lea    (%r9,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%r15,%rbp,8),%r8
mov    %r8,0x40(%r9,%rsi,1)
mov    %rdi,0x10(%r15,%rbp,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x34a9>
mov    %r9,%r13
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x358c>
lea    -0x4(%rax),%rcx
mov    0x88(%rsp),%r9
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
mov    %r13,%r9
lea    (%r9,%r8,1),%r10
mov    0x10(%r15,%rbp,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x10(%r15,%rbp,8)
lea    (%r9,%rdi,1),%r10
mov    0x10(%r15,%rbp,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%r15,%rbp,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%r15,%rbp,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%r15,%rbp,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%r15,%rbp,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%r15,%rbp,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3537>
mov    %r13,%rdi
add    $0x40,%rdi
mov    0x30(%rsp),%rdx
xor    %esi,%esi
mov    %rdi,%r15
call   *0x16dd9d(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r15,%r11
mov    0x58(%rsp),%rbp
mov    0x28(%rsp),%rdx
lea    0x1(%rdx),%r8
lea    0x20(,%r8,8),%eax
and    $0xffffffe0,%eax
cmp    $0xf,%dl
mov    $0xc0,%r13d
cmovne %rax,%r13
add    %r11,%r13
mov    0x90(%rsp),%r10
test   %r10,%r10
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3623>
lea    0x0(,%r10,8),%rdx
mov    %r11,%rdi
mov    0x38(%rsp),%rsi
mov    %r11,%r15
call   *0x16dc5b(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x90(%rsp),%rdx
imul   %r14,%rdx
mov    %r13,%rdi
mov    0x50(%rsp),%rsi
call   *0x16dc41(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r15,%r11
mov    0x28(%rsp),%rdx
mov    0x90(%rsp),%r10
lea    0x1(%rdx),%r8
mov    0x40(%rsp),%rax
mov    %rax,(%r11,%r10,8)
mov    0x20(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x551b>
lea    -0x20e803(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x186d>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %bl,0x0(%r13,%r10,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5561>
mov    %rdi,%rbp
mov    %r8,%rsi
mov    %r10,%r12
mov    %r10,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<5>>
lea    0x1(%r14),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37a0>
lea    (%rsi,%rsi,4),%rax
mov    0x0(%rbp,%rax,1),%ecx
movzbl 0x4(%rbp,%rax,1),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x373c>
mov    %rdi,%rbp
mov    %r8,%rsi
mov    %r10,%r12
mov    %r10,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<3>>
lea    0x1(%r14),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37a0>
lea    (%rsi,%rsi,2),%rax
movzwl 0x0(%rbp,%rax,1),%ecx
movzbl 0x2(%rbp,%rax,1),%eax
shl    $0x10,%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3740>
mov    %rdi,%rbp
mov    %r8,%rsi
mov    %r10,%r12
mov    %r10,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<4>>
lea    0x1(%r14),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37a0>
mov    0x0(%rbp,%rsi,4),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x370d>
mov    %rdi,%rbp
mov    %r8,%rsi
mov    %r10,%r12
mov    %r10,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<2>>
lea    0x1(%r14),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37a0>
movzwl 0x0(%rbp,%rsi,2),%eax
cmp    %rax,%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x378b>
mov    %rdi,%rbp
mov    %r8,%rsi
mov    %r10,%r12
mov    %r10,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<6>>
lea    0x1(%r14),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37a0>
lea    (%rsi,%rsi,2),%rax
mov    0x0(%rbp,%rax,2),%ecx
movzwl 0x4(%rbp,%rax,2),%eax
shl    $0x20,%rax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3788>
mov    %rdi,%rbp
mov    %r8,%rsi
mov    %r10,%r12
mov    %r10,%rdx
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<7>>
lea    0x1(%r14),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37a0>
lea    0x0(,%rsi,8),%rax
add    %rbp,%rax
sub    %rsi,%rax
mov    (%rax),%ecx
movzwl 0x4(%rax),%edx
movzbl 0x6(%rax),%eax
shl    $0x20,%rdx
or     %rcx,%rdx
shl    $0x30,%rax
or     %rdx,%rax
cmp    %r12,%rax
mov    0x18(%rsp),%r11
mov    0x48(%rsp),%rbp
mov    %r12,%r10
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d59>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37ad>
mov    0x18(%rsp),%r11
mov    0x48(%rsp),%rbp
mov    %r12,%r10
mov    %rsi,0x38(%rsp)
mov    0x20(%rsp),%rdx
cmp    $0x1,%rdx
sete   %r9b
cmp    $0x1,%edx
mov    $0x19,%eax
mov    $0x20,%ecx
cmovne %rcx,%rax
cmp    %rax,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3854>
mov    %rdx,%r13
lea    0x2(%r14),%rsi
test   %r14,%r14
mov    %rsi,0xf0(%rsp)
jle    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ab3>
cmp    $0xf,%r14
mov    0x1c0(%rsp),%r12
mov    0x38(%rsp),%r15
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3b00>
lea    0x5(%r14),%eax
and    $0x1c,%eax
cmp    $0x2,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3b11>
lea    0x8(%r13),%r11
lea    (%r11,%r11,1),%ecx
mov    %rcx,0xd0(%rsp)
mov    0x50(%rsp),%rdx
mov    (%rdx),%ecx
mov    0x3(%rdx),%edx
mov    %edx,0x9b(%rsp)
mov    %ecx,0x98(%rsp)
mov    $0x2,%ecx
mov    %rcx,0x78(%rsp)
mov    0x8(%rsp),%rdi
mov    %r9d,%edx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3fa3>
mov    %rax,0x78(%rsp)
mov    %r10,0x30(%rsp)
mov    0x50(%rsp),%r12
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %ecx,0x9b(%rsp)
mov    %eax,0x98(%rsp)
lea    0xd8(%rsp),%rdi
mov    0x10(%rsp),%rbp
mov    %rbp,%rsi
mov    %rdx,0x20(%rsp)
mov    %r8,%rcx
call   *0x16dd40(%rip)        # ADDR <_DYNAMIC+0x5a0>
mov    0x0(%rbp),%rax
mov    %rax,0x18(%rsp)
and    $0xfffffffffffffffc,%r14
add    $0x4,%r14
cmpq   $0x21,0x88(%rsp)
mov    $0x20,%eax
cmovb  %rax,%r14
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %ecx,0xfb(%rsp)
mov    %eax,0xf8(%rsp)
mov    0xe8(%rsp),%rbp
cmp    0x38(%rsp),%rbp
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62ba>
cmp    0xd8(%rsp),%rbp
mov    0x1c0(%rsp),%r12
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3905>
lea    0xd8(%rsp),%rdi
call   *0x16e283(%rip)        # ADDR <_DYNAMIC+0xb50>
mov    0x38(%rsp),%rax
mov    %rax,%rcx
shl    $0x4,%rcx
add    0xe0(%rsp),%rcx
mov    %rbp,%rdx
sub    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3942>
lea    0x10(%rcx),%rdi
shl    $0x4,%rdx
mov    %rbp,0x38(%rsp)
mov    %rcx,%rbp
mov    %rcx,%rsi
call   *0x16d9e6(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,%rcx
mov    0x38(%rsp),%rbp
lea    0x1(%rbp),%r8
mov    0x30(%rsp),%rax
mov    %rax,(%rcx)
mov    0x40(%rsp),%rax
mov    %rax,0x8(%rcx)
mov    %r8,0xe8(%rsp)
mov    0x28(%rsp),%rax
mov    0x20(%rsp),%r10
cmp    %r10b,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a30>
mov    %r10,%rsi
xor    $0x7,%rsi
sub    %r10b,%al
movzbl %al,%edi
dec    %rdi
lea    -0x4(%r13),%eax
movzbl %al,%eax
mov    0x10(%rsp),%rcx
add    %rax,%rcx
add    $0x7,%rcx
xor    %eax,%eax
lea    0x162db2(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
xchg   %ax,%ax
cmp    %rsi,%rdi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64f0>
mov    %rax,%r9
shl    $0x8,%r9
movzbl 0x1(%rcx,%rdi,1),%eax
or     %r9,%rax
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x39a0>
mov    %r15d,%ecx
shl    %cl,%rax
mov    0xe0(%rsp),%rcx
mov    %r8,%rsi
mov    %rcx,%rdx
and    $0x7,%rsi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x39ec>
mov    %rcx,%rdx
nopl   0x0(%rax,%rax,1)
or     %rax,(%rdx)
add    $0x10,%rdx
dec    %rsi
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x39e0>
cmp    $0x7,%rbp
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a28>
shl    $0x4,%r8
add    %r8,%rcx
nopl   0x0(%rax)
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
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a00>
mov    0xe8(%rsp),%r8
mov    0xe0(%rsp),%rcx
cmp    0x78(%rsp),%r8
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a60>
cmp    $0x1,%r10d
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3c02>
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%rsi
mov    %rcx,%rdx
mov    %r8,%rcx
call   ADDR <expanse_trie::mutate_map::build_bitmap_leaf_map::<false>>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a72>
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%rsi
mov    %r10d,%edx
call   ADDR <expanse_trie::mutate_map::build_map_leaf::<false>>
mov    0x20(%rsp),%r15
cmp    %r15b,0x28(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5319>
mov    0x28(%rsp),%rax
movzbl %al,%edx
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6343>
sub    %r15,%rdx
lea    (%rsp,%r15,1),%rsi
add    $0xf8,%rsi
mov    0x50(%rsp),%rdi
add    %r15,%rdi
call   *0x16d7a2(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5319>
lea    0x8(%r13),%r11
mov    %r8,%rax
imul   %r11,%rax
mov    %rax,0xd0(%rsp)
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x9b(%rsp)
mov    %eax,0x98(%rsp)
mov    %r8,0x78(%rsp)
mov    %rsi,%rax
mov    0x1c0(%rsp),%r12
mov    0x8(%rsp),%rdi
mov    0x38(%rsp),%r15
mov    %r9d,%edx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3fa3>
xor    %eax,%eax
cmp    $0x17,%r14
setae  %al
lea    0x18(,%rax,8),%rax
xor    %ecx,%ecx
cmp    $0x19,%r8
setae  %cl
movabs $0x7ffffffffffffff0,%rdx
or     $0xc,%rdx
and    %r14,%rdx
add    $0x4,%rdx
cmp    $0x11,%r8
lea    0x18(,%rcx,8),%rcx
cmovae %rcx,%rdx
cmp    %rdx,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d36>
add    $0x4,%r14d
and    $0x3c,%r14d
cmp    $0x11,%r8
cmovb  %r14d,%ecx
lea    (%r11,%rcx,8),%rdi
lea    -0x1(%r13),%eax
cmp    $0x5,%eax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x59d1>
lea    -0x20ec90(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x190d>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    %r8,%r12
sub    %r15,%r12
mov    0x58(%rsp),%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3beb>
lea    (%r11,%r15,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %r10,0x30(%rsp)
lea    0x0(,%r12,8),%rdx
mov    0x16d775(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,0x48(%rsp)
mov    %rax,%rbp
call   *%rax
mov    0x8(%rsp),%rax
lea    (%rax,%r15,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r15,1),%rdi
inc    %rdi
mov    %r12,%rdx
call   *%rbp
mov    0x8(%rsp),%rdi
mov    0x88(%rsp),%r8
mov    0x30(%rsp),%r10
mov    0x48(%rsp),%rbp
mov    0x18(%rsp),%r11
mov    0x40(%rsp),%rax
mov    %rax,(%r11,%r15,8)
mov    %r10b,(%rdi,%r15,1)
mov    0x10(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f20>
shl    $0x4,%r8
mov    -0x10(%rcx,%r8,1),%rdx
xor    (%rcx),%rdx
mov    0x28(%rsp),%rax
cmp    $0x7,%al
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3c36>
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
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d92>
add    %rcx,%r8
lea    0xa8(%rsp),%rdi
mov    %rcx,%rsi
mov    %r8,%rdx
call   ADDR <<alloc::vec::Vec<(u64, u64)> as alloc::vec::spec_from_iter::SpecFromIter<(u64, u64), core::iter::adapters::map::Map<core::slice::iter::Iter<(u64, u64)>, expanse_trie::mutate_map::map_insert_with_path_occ<false, true, false>::{closure#0}>>>::from_iter>
cmpq   $0x0,0xe8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6586>
mov    0xe0(%rsp),%rax
mov    (%rax),%rbp
mov    0xb0(%rsp),%rdx
mov    0xb8(%rsp),%rcx
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%r13
mov    %r13,%rsi
call   ADDR <expanse_trie::mutate_map::build_bitmap_leaf_map::<false>>
mov    0x28(%rsp),%r15
movzbl %r15b,%edx
mov    %r13,%rdi
mov    $0x1,%esi
mov    %rbp,%rcx
call   *0x16d923(%rip)        # ADDR <_DYNAMIC+0x5a8>
cmp    $0x1,%r15b
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d0f>
mov    %rbx,%rax
shr    $0x8,%rax
mov    %rax,0x40(%r12)
mov    0x10(%rsp),%rcx
mov    (%rcx),%rax
mov    %rax,0x58(%r12)
movq   $0x0,0x60(%r12)
movzwl 0xe8(%rsp),%eax
mov    %ax,0x70(%r12)
mov    %rcx,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
cmpq   $0x0,0xa8(%rsp)
mov    0x20(%rsp),%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5319>
mov    0xb0(%rsp),%rdi
call   *0x16d537(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5319>
cmp    $0x11,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3de6>
lea    0x4(%r14),%eax
and    $0x3c,%eax
lea    0x8(%r13),%r11
imul   %r11,%rax
mov    %rax,0xd0(%rsp)
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x9b(%rsp)
mov    %eax,0x98(%rsp)
cmp    $0xe,%r14
mov    %r9d,%edx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f88>
lea    0x5(%r14),%eax
and    $0x1c,%eax
mov    %r8,0x78(%rsp)
mov    0x8(%rsp),%rdi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3fa3>
mov    $0x3f,%cl
sub    %al,%cl
shr    $0x3,%cl
inc    %cl
mov    0x28(%rsp),%r12
cmp    $0x8,%r12b
movzbl %cl,%r15d
movzbl %r12b,%eax
mov    %eax,%ebp
cmovae %eax,%r15d
mov    0x8(%rsp),%rdx
addq   $0x40,0x200(%rdx)
incq   0x208(%rdx)
incq   0x218(%rdx)
mov    0x18(%rdx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5177>
mov    (%rax),%rcx
mov    %rcx,0x18(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x51e6>
lea    0x8(%r13),%r11
mov    0x50(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x9b(%rsp)
mov    %eax,0x98(%rsp)
cmp    $0x18,%r8
mov    %r9d,%edx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f7a>
lea    0x0(,%r11,8),%eax
lea    (%rax,%rax,2),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f80>
mov    0x40(%rsp),%rax
mov    %rax,(%rsi)
mov    0x50(%rsp),%r12
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %ecx,0x73(%rsp)
mov    %eax,0x70(%rsp)
mov    0x58(%rsp),%r10
mov    0x20(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c00>
lea    -0x20efe2(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x18a5>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %bl,0x70(%rsp,%r11,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c00>
mov    %rdi,0x50(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
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
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f18>
lea    -0x30(%rcx),%rsi
mov    0x80(%r8),%rdi
mov    %rdi,-0x30(%rcx)
mov    %rsi,0x80(%r8)
add    $0xffffffffffffffc0,%rcx
add    $0x4,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3eb6>
add    $0x40,%rax
mov    0x50(%rsp),%rdi
mov    0x40(%rsp),%rcx
mov    %rcx,(%rax)
mov    %r12,0x8(%rax)
movw   $0x0,0x64(%rsp)
movl   $0x0,0x60(%rsp)
movzbl %r14b,%ecx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a9e>
lea    -0x20f0be(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0x18c1>
movslq (%rdx,%rcx,4),%rcx
add    %rdx,%rcx
mov    0x1c0(%rsp),%r11
mov    0x68(%rsp),%rdx
jmp    *%rcx
mov    %r15b,0x60(%rsp)
mov    %bpl,0x61(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ae9>
mov    %r11d,%eax
shl    $0x5,%eax
mov    %rax,0xd0(%rsp)
mov    0x8(%rsp),%rdi
xor    %eax,%eax
cmp    $0x17,%r14
setae  %al
lea    0x18(,%rax,8),%rax
mov    %r8,0x78(%rsp)
mov    %r15,0x38(%rsp)
imul   %rax,%r11
lea    0xf(%r11),%rsi
and    $0xfffffffffffffff0,%rsi
cmp    $0x178,%r11
mov    %rbp,0x48(%rsp)
mov    %r10,0x30(%rsp)
mov    %dl,0x9f(%rsp)
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4115>
lea    -0x202051(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%r11,%rax,1),%r15d
cmp    $0xff,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65cd>
add    %rsi,0x200(%rdi)
incq   0x208(%rdi)
incq   0x218(%rdi)
cmp    $0x3d,%r15b
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6537>
mov    0x10(%rdi,%r15,8),%r9
test   %r9,%r9
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4024>
mov    (%r9),%rax
mov    %rax,0x10(%rdi,%r15,8)
mov    %r13,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x40fa>
cmp    $0x101,%r11
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4121>
mov    %rsi,0x180(%rsp)
mov    %r11,0x20(%rsp)
movq   $0x0,0xa8(%rsp)
lea    0xa8(%rsp),%rdi
mov    $0x40,%esi
mov    $0x1000,%edx
call   *0x16d2ce(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
mov    %r13,%rbp
mov    0xa8(%rsp),%r13
test   %r13,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
mov    $0x1000,%edx
mov    %r13,%rdi
xor    %esi,%esi
call   *0x16d2b2(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r13,%r9
mov    0x8(%rsp),%rcx
mov    0x220(%rcx),%rax
mov    %rax,0x0(%r13)
mov    %r15,0x8(%r13)
mov    %r13,0x220(%rcx)
mov    $0xfc0,%ax
xor    %edx,%edx
mov    0x180(%rsp),%rdi
div    %di
movzwl %ax,%eax
dec    %rax
mov    %rax,%rcx
imul   %rdi,%rcx
lea    (%r9,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x8(%rsp),%rsi
mov    0x10(%rsi,%r15,8),%rsi
mov    %rsi,0x40(%r9,%rcx,1)
mov    0x8(%rsp),%rcx
mov    %rdx,0x10(%rcx,%r15,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x40c0>
add    $0x40,%r9
mov    0x20(%rsp),%r11
mov    %r9,%rdi
xor    %esi,%esi
mov    %r11,%rdx
mov    %r9,%r15
call   *0x16d235(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r15,%r10
mov    0x38(%rsp),%r15
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4148>
mov    %r11,%r15
call   *0x16d41a(%rip)        # ADDR <_DYNAMIC+0x500>
mov    %r15,%r11
mov    %r11,0x20(%rsp)
mov    $0x1,%esi
mov    %r11,%rdi
call   *0x16d204(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r10
test   %rax,%rax
mov    0x38(%rsp),%r15
mov    %r13,%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66b4>
lea    -0x1(%rbp),%eax
cmp    $0x5,%eax
mov    0x18(%rsp),%r8
mov    %r10,0x20(%rsp)
lea    0x1(%r14),%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4468>
lea    -0x20f271(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x1925>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
mov    0x40(%rsp),%r13
jmp    *%rax
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x419e>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16d0c0(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
mov    %r13,(%r10,%r15,8)
mov    0x78(%rsp),%r13
mov    %r13,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x41d3>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16d08b(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
cmp    $0x3,%rsi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4580>
mov    %r13,%rax
lea    (%r8,%r13,8),%r13
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x45a9>
add    $0x2,%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b27>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x421e>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16d040(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
mov    %r13,(%r10,%r15,8)
mov    0x78(%rsp),%r13
mov    %r13,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4253>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16d00b(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
cmp    $0x3,%rsi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44ea>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x450f>
add    $0x2,%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x497c>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x429b>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16cfc3(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
mov    %r13,(%r10,%r15,8)
mov    0x78(%rsp),%r13
mov    %r13,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x42d0>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16cf8e(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
cmp    $0x3,%rsi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x451c>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4541>
add    $0x2,%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a0e>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4318>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16cf46(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
mov    %r13,(%r10,%r15,8)
mov    0x78(%rsp),%r13
mov    %r13,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x434d>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16cf11(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
cmp    $0x3,%rsi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x454e>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4573>
add    $0x2,%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a9b>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4395>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16cec9(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
mov    %r13,(%r10,%r15,8)
mov    0x78(%rsp),%r13
mov    %r13,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x43ca>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16ce94(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
cmp    $0x3,%rsi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x45b6>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x45db>
add    $0x2,%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ba6>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4412>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16ce4c(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
mov    %r13,(%r10,%r15,8)
mov    0x78(%rsp),%r13
mov    %r13,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4447>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16ce17(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
cmp    $0x3,%rsi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x45e8>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x460d>
add    $0x2,%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c2c>
test   %r15,%r15
mov    0x40(%rsp),%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4494>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16cdca(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
mov    %r13,(%r10,%r15,8)
mov    0x78(%rsp),%r13
mov    %r13,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44c9>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16cd95(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    0x1(%r14),%rsi
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
cmp    $0x3,%rsi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x461a>
lea    (%r8,%r13,8),%rsi
cmp    $0x2,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x463f>
add    $0x2,%r14
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4cc6>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4954>
lea    0x20(,%r14,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x496b>
add    $0x5,%r14d
and    $0x1c,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x497c>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49e6>
lea    0x20(,%r14,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49fd>
add    $0x5,%r14d
and    $0x1c,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a0e>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a73>
lea    0x20(,%r14,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a8a>
add    $0x5,%r14d
and    $0x1c,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a9b>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4afd>
mov    %r13,%rax
lea    0x20(,%r14,8),%r13d
and    $0x1e0,%r13d
add    %r8,%r13
cmp    $0x10,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b14>
add    $0x5,%r14d
and    $0x1c,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b27>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b7e>
lea    0x20(,%r14,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b95>
add    $0x5,%r14d
and    $0x1c,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ba6>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c04>
lea    0x20(,%r14,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c1b>
add    $0x5,%r14d
and    $0x1c,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c2c>
cmp    $0x11,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c9e>
lea    0x20(,%r14,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4cb5>
add    $0x5,%r14d
and    $0x1c,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4cc6>
mov    %r10,%r12
mov    %rdx,0x48(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6333>
mov    %rax,%r10
mov    0x8(%rsp),%rcx
mov    0x220(%rcx),%rax
mov    %rax,(%r10)
mov    %rbp,0x8(%r10)
mov    %r10,0x220(%rcx)
mov    $0xfc0,%ax
xor    %edx,%edx
divw   0x48(%rsp)
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
mov    %r12,%r11
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x46e3>
mov    %r11d,%eax
and    $0xffffffe0,%eax
lea    -0x1(%rcx),%rsi
imul   %rax,%rsi
add    %r10,%rsi
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
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x46c5>
mov    %rcx,%rax
sub    %rdi,%rax
mov    %r10,0x30(%rsp)
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
mov    0x8(%rsp),%r12
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47a1>
lea    -0x4(%rax),%rcx
shr    $0x5,%r11d
imul   %r11,%rcx
shl    $0x5,%rcx
add    $0x40,%rcx
mov    %r11d,%edx
shl    $0x7,%edx
neg    %rdx
lea    -0x3(%rax),%rsi
imul   %r11,%rsi
shl    $0x5,%rsi
add    $0x40,%rsi
lea    -0x2(%rax),%rdi
imul   %r11,%rdi
shl    $0x5,%rdi
add    $0x40,%rdi
lea    -0x1(%rax),%r8
imul   %r11,%r8
shl    $0x5,%r8
add    $0x40,%r8
mov    0x30(%rsp),%r9
lea    (%r9,%r8,1),%r10
mov    0x10(%r12,%rbp,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x10(%r12,%rbp,8)
lea    (%r9,%rdi,1),%r10
mov    0x10(%r12,%rbp,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%r12,%rbp,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%r12,%rbp,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%r12,%rbp,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%r12,%rbp,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%r12,%rbp,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x474c>
mov    0x30(%rsp),%rdi
add    $0x40,%rdi
mov    0x1c0(%rsp),%r12
mov    0x48(%rsp),%rdx
mov    %rdi,0x30(%rsp)
xor    %esi,%esi
call   *0x16cb7c(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x90(%rsp),%r8
test   %r8,%r8
mov    0x30(%rsp),%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47f4>
lea    0x0(,%r8,8),%rdx
mov    %rbp,%rdi
mov    0x38(%rsp),%rsi
call   *0x16ca64(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x90(%rsp),%r8
lea    0x0(,%r8,8),%r10
add    %rbp,%r10
mov    0x40(%rsp),%rax
mov    %rax,0x0(%rbp,%r8,8)
mov    0x28(%rsp),%rsi
lea    0x1(%rsi),%rdx
cmp    %r8,%rdx
mov    %rbp,%r9
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x484f>
mov    0x38(%rsp),%rax
lea    (%rax,%r8,8),%rsi
lea    0x8(%r10),%rdi
sub    %r8d,%edx
shl    $0x3,%edx
mov    %r10,%rbp
call   *0x16ca1a(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,%r10
mov    0x30(%rsp),%r9
mov    0x28(%rsp),%rsi
mov    0x90(%rsp),%r8
lea    0x1(%rsi),%rdx
mov    0x10(%rsp),%rax
mov    (%rax),%rbp
test   %rbp,%rbp
mov    0x8(%rsp),%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63b9>
mov    0x78(%rsp),%rax
add    $0x18,%eax
and    $0xffffffe0,%eax
cmp    $0x1,%sil
mov    $0x10,%esi
cmovne %rax,%rsi
lea    -0x2028fc(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rsi,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6779>
sub    %rsi,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6525>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,0x0(%rbp)
mov    %rbp,0x10(%rcx,%rdi,8)
mov    0x50(%rsp),%rsi
mov    (%rsi),%eax
mov    0x3(%rsi),%ecx
mov    %ecx,0x83(%rsp)
mov    %eax,0x80(%rsp)
cmp    %r8,%rdx
mov    0x58(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x491f>
mov    %r8,%rdx
imul   %r14,%rdx
inc    %r8
imul   %r14,%r8
lea    0x1635d6(%rip),%r9        # ADDR <anon.ADDR.2.llvm.ADDR+0x958>
lea    0x80(%rsp),%rdi
mov    $0x7,%esi
mov    %r15,%rcx
mov    %r10,%r15
call   ADDR <<[u8]>::copy_within::<core::ops::range::Range<usize>>>
mov    %r15,%r10
mov    0x30(%rsp),%r9
mov    0x90(%rsp),%r8
mov    0x50(%rsp),%rsi
mov    0x20(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
mov    0x88(%rsp),%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d64>
lea    -0x20fae2(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x1889>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %bl,0x80(%rsp,%r8,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d64>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%r14
lea    (%r10,%r14,8),%r15
mov    0x38(%rsp),%r14
test   %r14,%r14
mov    %rsi,0x40(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49ab>
lea    (%r14,%r14,4),%rbp
mov    %r15,%rdi
mov    %rbp,%rdx
call   *0x16c8b1(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49ad>
xor    %ebp,%ebp
mov    0x30(%rsp),%rdx
mov    %rdx,%rcx
shr    $0x20,%rcx
mov    %r13,%rax
mov    %edx,(%r15,%rbp,1)
mov    %cl,0x4(%r15,%rbp,1)
sub    %r14,%rax
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d55>
add    %rbp,%r15
mov    0x40(%rsp),%rsi
add    %rbp,%rsi
add    $0x5,%r15
lea    (%rax,%rax,4),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d42>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%r14
lea    (%r10,%r14,8),%r15
mov    0x38(%rsp),%r14
test   %r14,%r14
mov    %rsi,0x40(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a3d>
lea    (%r14,%r14,2),%rbp
mov    %r15,%rdi
mov    %rbp,%rdx
call   *0x16c81f(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a3f>
xor    %ebp,%ebp
mov    0x30(%rsp),%rdx
mov    %edx,%ecx
shr    $0x10,%ecx
mov    %r13,%rax
mov    %dx,(%r15,%rbp,1)
mov    %cl,0x2(%r15,%rbp,1)
sub    %r14,%rax
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d55>
add    %rbp,%r15
mov    0x40(%rsp),%rsi
add    %rbp,%rsi
add    $0x3,%r15
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c95>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%r14
lea    (%r10,%r14,8),%r15
mov    0x38(%rsp),%r14
test   %r14,%r14
mov    %rsi,0x40(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ace>
lea    0x0(,%r14,4),%rbp
mov    %r15,%rdi
mov    %rbp,%rdx
call   *0x16c78e(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ad0>
xor    %ebp,%ebp
mov    %r13,%rdx
mov    0x30(%rsp),%rax
mov    %eax,(%r15,%rbp,1)
sub    %r14,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d55>
add    %rbp,%r15
mov    0x40(%rsp),%rsi
add    %rbp,%rsi
add    $0x4,%r15
shl    $0x2,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d42>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%r13
add    $0xc0,%r13
xor    %eax,%eax
cmpq   $0x18,0x78(%rsp)
setae  %al
lea    0x18(,%rax,8),%r14
lea    (%r10,%r14,8),%r15
mov    0x38(%rsp),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b4e>
mov    %r15,%rdi
mov    %r13,%rsi
mov    %rbp,%rdx
call   *0x16c70c(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
mov    0x78(%rsp),%rdx
mov    0x30(%rsp),%rax
mov    %al,(%r15,%rbp,1)
sub    %rbp,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d55>
mov    0x38(%rsp),%rax
add    %rax,%r15
add    %rax,%r13
inc    %r15
mov    %r15,%rdi
mov    %r13,%rsi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d45>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%r14
lea    (%r10,%r14,8),%r15
mov    0x38(%rsp),%r14
test   %r14,%r14
mov    %rsi,0x40(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bd5>
lea    (%r14,%r14,1),%rbp
mov    %r15,%rdi
mov    %rbp,%rdx
call   *0x16c687(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bd7>
xor    %ebp,%ebp
mov    %r13,%rdx
mov    0x30(%rsp),%rax
mov    %ax,(%r15,%rbp,1)
sub    %r14,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d55>
add    %rbp,%r15
mov    0x40(%rsp),%rsi
add    %rbp,%rsi
add    $0x2,%r15
add    %rdx,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d42>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%r14
lea    (%r10,%r14,8),%r15
mov    0x38(%rsp),%r14
test   %r14,%r14
mov    %rsi,0x40(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c5f>
lea    (%r14,%r14,1),%rax
lea    (%rax,%rax,2),%rbp
mov    %r15,%rdi
mov    %rbp,%rdx
call   *0x16c5fd(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c61>
xor    %ebp,%ebp
mov    0x30(%rsp),%rdx
mov    %rdx,%rcx
shr    $0x20,%rcx
mov    %r13,%rax
mov    %edx,(%r15,%rbp,1)
mov    %cx,0x4(%r15,%rbp,1)
sub    %r14,%rax
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d55>
add    %rbp,%r15
mov    0x40(%rsp),%rsi
add    %rbp,%rsi
add    $0x6,%r15
add    %rax,%rax
lea    (%rax,%rax,2),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d42>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r13
setae  %al
lea    0x18(,%rax,8),%r14
lea    (%r10,%r14,8),%r15
mov    0x38(%rsp),%r14
test   %r14,%r14
mov    %rsi,0x40(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4cfc>
lea    0x0(,%r14,8),%rbp
sub    %r14,%rbp
mov    %r15,%rdi
mov    %rbp,%rdx
call   *0x16c560(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4cfe>
xor    %ebp,%ebp
mov    0x30(%rsp),%rsi
mov    %rsi,%rcx
shr    $0x20,%rcx
mov    %rsi,%rdx
shr    $0x30,%rdx
mov    %r13,%rax
mov    %esi,(%r15,%rbp,1)
mov    %cx,0x4(%r15,%rbp,1)
mov    %dl,0x6(%r15,%rbp,1)
sub    %r14,%rax
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d55>
add    %rbp,%r15
mov    0x40(%rsp),%rsi
add    %rbp,%rsi
add    $0x7,%r15
lea    0x0(,%rax,8),%rdx
sub    %rax,%rdx
mov    %r15,%rdi
call   *0x16c505(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%r10
mov    0x18(%rsp),%r8
mov    0x10(%rsp),%rdx
mov    %r10,(%rdx)
mov    0x98(%rsp),%eax
mov    0x9b(%rsp),%ecx
mov    0x50(%rsp),%rsi
mov    %ecx,0x3(%rsi)
mov    %rdx,%rcx
mov    %eax,(%rsi)
mov    0x8(%rdx),%rax
mov    0x78(%rsp),%rdx
xor    %rax,%rdx
and    0x48(%rsp),%rdx
xor    %rax,%rdx
mov    %rdx,0x8(%rcx)
test   %r8,%r8
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62a1>
mov    0xd0(%rsp),%rdx
lea    0xf(%rdx),%rsi
and    $0xfffffffffffffff0,%rsi
cmp    $0x177,%rdx
mov    0x58(%rsp),%r14
mov    0x38(%rsp),%r15
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e70>
lea    -0x202e3f(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rdx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e70>
mov    0x8(%rsp),%rdx
sub    %rsi,0x200(%rdx)
decq   0x208(%rdx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6525>
mov    0x10(%rdx,%rdi,8),%rax
mov    %rax,(%r8)
mov    %r8,0x10(%rdx,%rdi,8)
cmpb   $0x1,0x28(%rsp)
sete   %al
test   %al,0x9f(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ea6>
shr    $0x8,%rbx
mov    %rbx,0x40(%r12)
movq   $0x0,0x58(%r12)
mov    %r10,0x60(%r12)
mov    0xf0(%rsp),%rax
mov    %ax,0x70(%r12)
mov    %rcx,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
mov    $0x1,%edx
mov    0x68(%rsp),%rax
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x50bd>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x515b>
mov    0x8(%rsp),%rdi
mov    %r8,%r13
call   *0x16c6f2(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %r13,%rdi
call   *0x16c3e1(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x20(%rsp),%r10
mov    0x10(%rsp),%rcx
cmpb   $0x1,0x28(%rsp)
sete   %al
test   %al,0x9f(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e17>
mov    (%r14),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x50ad>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x508b>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x508b>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f09>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205087(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x508b>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f49>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2050c7(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x508b>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f89>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205107(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x508b>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4fc9>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205147(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x508b>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5009>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x205187(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x508b>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5045>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2051c3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x508b>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5081>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2051ff(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r14)
movq   $0x0,0x10(%r14)
xor    %edx,%edx
mov    0x68(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x515b>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x100,%rax
lea    0x100(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x50fb>
data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %sil,0x48(%r12,%rdi,1)
mov    0x50(%r12),%rdx
inc    %rdx
mov    %rdx,0x50(%r12)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x515b>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5130>
mov    0x8(%rdi),%rdx
lea    -0x2052ab(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rsi,8),%r8
mov    %r8,%r9
and    %rdx,%r9
inc    %r9
xor    %rdx,%r9
and    %r8,%r9
xor    %rdx,%r9
mov    %r9,0x8(%rdi)
mov    (%r14),%rdx
lea    -0x1(%rdx),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x50f2>
mov    %rdi,(%r12,%rdx,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x50e0>
lea    0x169758(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
mov    $0x8,%esi
call   *0x16c0e5(%rip)        # ADDR <_DYNAMIC+0x208>
lea    (%r10,%r15,8),%rax
mov    0xa0(%rsp),%rcx
movq   $0x0,(%rcx)
mov    %rax,0x10(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54da>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6454>
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
jg     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x51b9>
add    $0x40,%rax
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rax)
movdqu %xmm0,0x30(%rax)
movdqu %xmm0,0x20(%rax)
movdqu %xmm0,0x10(%rax)
mov    %r15b,0x5(%rax)
movl   $0x0,0xab(%rsp)
movl   $0x0,0xa8(%rsp)
mov    0x10(%rsp),%rdx
mov    %rax,(%rdx)
mov    0xa8(%rsp),%eax
mov    0xab(%rsp),%ecx
mov    0x50(%rsp),%rsi
mov    %ecx,0x3(%rsi)
mov    %eax,(%rsi)
movb   $0x1,0xf(%rdx)
movzbl %r15b,%r13d
cmp    %r12b,%r15b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x526e>
cmpq   $0x0,0xe8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65aa>
mov    0xe0(%rsp),%rax
mov    (%rax),%rcx
mov    0x10(%rsp),%rdi
mov    %r13d,%esi
mov    %ebp,%edx
call   *0x16c372(%rip)        # ADDR <_DYNAMIC+0x5a8>
mov    0xe8(%rsp),%r12
test   %r12,%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52e1>
mov    %r13,0x40(%rsp)
mov    0xe0(%rsp),%r13
shl    $0x4,%r12
add    %r13,%r12
lea    0xa8(%rsp),%r15
nopw   0x0(%rax,%rax,1)
mov    0x0(%r13),%rcx
mov    0x8(%r13),%r8
mov    0x1c0(%rsp),%rax
mov    %rax,(%rsp)
mov    %r15,%rdi
mov    0x8(%rsp),%rsi
mov    0x10(%rsp),%rdx
mov    %ebp,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
add    $0x10,%r13
cmp    %r12,%r13
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52a0>
mov    0xe8(%rsp),%rax
mov    0x40(%rsp),%r13
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52e3>
xor    %eax,%eax
cmp    $0x8,%r13b
mov    0x1c0(%rsp),%r12
mov    0x20(%rsp),%r15
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6599>
dec    %rax
mov    0x50(%rsp),%rsi
mov    (%rsi),%rcx
xor    %rcx,%rax
lea    -0x2054a7(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rdx,%r13,8),%rax
xor    %rcx,%rax
mov    %rax,(%rsi)
cmpq   $0x0,0x18(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62d4>
lea    0x8(%r15),%rax
imul   %rax,%r14
lea    0xc(%r14),%rsi
and    $0xfffffffffffffff0,%rsi
cmp    $0x177,%r14
mov    0x8(%rsp),%rcx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x542b>
lea    -0x2033c6(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%r14,%rax,1),%edi
cmp    $0xff,%rdi
mov    0x68(%rsp),%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6680>
sub    %rsi,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
mov    0x58(%rsp),%r11
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6561>
mov    0x10(%rcx,%rdi,8),%rax
mov    0x18(%rsp),%rdx
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5465>
shl    $0x4,%r14
lea    (%rsp,%r14,1),%rax
add    $0x100,%rax
lea    0x100(%rsp),%rcx
lea    0x1694ee(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x53cd>
nopl   0x0(%rax)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5465>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x53ff>
mov    0x8(%rdi),%r8
lea    -0x20557d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x53c0>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64eb>
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x53c0>
mov    %rcx,%rdi
call   *0x16c13c(%rip)        # ADDR <_DYNAMIC+0x538>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r14
mov    0x68(%rsp),%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6412>
mov    0x18(%rsp),%rdi
call   *0x16be11(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x58(%rsp),%r11
test   %r14,%r14
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x539d>
mov    0x16c0b4(%rip),%rax        # ADDR <_DYNAMIC+0x4e8>
movzbl (%rax),%eax
cmp    $0x2,%al
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5487>
movzbl 0x28(%rsp),%edx
mov    0x10(%rsp),%rdi
mov    %rbx,%rsi
call   ADDR <expanse_trie::get::locate_slot_popcnt>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54a5>
movzbl %al,%eax
cmp    $0x1,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6320>
movzbl 0x28(%rsp),%edx
mov    0x10(%rsp),%rdi
mov    %rbx,%rsi
call   ADDR <expanse_trie::get::locate_slot_swar>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62e9>
mov    0xa0(%rsp),%rcx
movq   $0x0,(%rcx)
mov    %rax,0x10(%rcx)
cmpq   $0x0,0xd8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54da>
mov    0xe0(%rsp),%rdi
call   *0x16bd8e(%rip)        # ADDR <free@GLIBC_2.2.5>
add    $0x188,%rsp
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
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5561>
lea    (%r10,%r10,2),%rax
mov    %bx,0x0(%r13,%rax,1)
shr    $0x10,%ebx
mov    %bl,0x2(%r13,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5561>
mov    %ebx,0x0(%r13,%r10,4)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5561>
mov    %rbx,0x0(%r13,%r10,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5561>
mov    %bx,0x0(%r13,%r10,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5561>
lea    (%r10,%r10,2),%rax
mov    %ebx,0x0(%r13,%rax,2)
shr    $0x20,%rbx
mov    %bx,0x4(%r13,%rax,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5561>
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
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x55d7>
mov    0x38(%rsp),%rax
lea    (%rax,%r10,8),%rsi
lea    0x1(%r10),%r12
lea    (%r11,%r10,8),%rdi
add    $0x8,%rdi
lea    0x0(,%rbx,8),%edx
mov    0x16bcc4(%rip),%r15        # ADDR <memcpy@GLIBC_2.14>
mov    %r11,0x58(%rsp)
call   *%r15
mov    0x90(%rsp),%rsi
imul   %r14,%rsi
add    0x50(%rsp),%rsi
imul   %r14,%r12
add    %r12,%r13
mov    0x1c0(%rsp),%r12
imul   %r14,%rbx
mov    %r13,%rdi
mov    %rbx,%rdx
call   *%r15
mov    0x58(%rsp),%r11
mov    0x28(%rsp),%rdx
mov    0x90(%rsp),%r10
lea    0x1(%rdx),%r8
mov    0x10(%rsp),%rcx
mov    (%rcx),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6360>
lea    0x0(,%r8,8),%eax
add    $0x18,%eax
and    $0xffffffe0,%eax
cmp    $0x1,%dl
mov    $0x10,%esi
cmovne %rax,%rsi
lea    -0x203681(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rsi,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6707>
mov    0x8(%rsp),%rdx
sub    %rsi,0x200(%rdx)
decq   0x208(%rdx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6525>
mov    0x10(%rdx,%rdi,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0x10(%rdx,%rdi,8)
lea    0x4(%r14),%eax
movl   $0x0,0xab(%rsp)
movl   $0x0,0xa8(%rsp)
mov    %r11,(%rcx)
mov    %rcx,%rsi
mov    0xa8(%rsp),%ecx
mov    0xab(%rsp),%edx
mov    0x50(%rsp),%rdi
mov    %edx,0x3(%rdi)
mov    %ecx,(%rdi)
mov    %al,0xf(%rsi)
mov    0x8(%rsi),%rax
xor    %rax,%r8
lea    -0x205825(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rcx,%r14,8),%r8
xor    %rax,%r8
mov    %r8,0x8(%rsi)
mov    0x68(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5727>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x100,%rax
lea    0x100(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x56d3>
nopl   (%rax)
mov    %dl,0x48(%r12,%rdi,1)
incq   0x50(%r12)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5727>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5705>
mov    0x8(%rsi),%rdi
lea    -0x205883(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x56ca>
mov    %rsi,(%r12,%rdi,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x56c0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5149>
lea    (%r11,%r10,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x515f>
mov    %r8,%r12
sub    %r15,%r12
mov    0x58(%rsp),%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e3b>
lea    (%r11,%r15,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %r10,0x30(%rsp)
lea    0x0(,%r12,8),%rdx
mov    %rbp,0x48(%rsp)
call   *0x16bbb6(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r15,%rbp
lea    (%r15,%r15,4),%r15
mov    0x8(%rsp),%rax
lea    (%rax,%r15,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r15,1),%rdi
add    $0x5,%rdi
lea    (%r12,%r12,4),%rdx
call   *0x16bb8f(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,%rsi
mov    0x8(%rsp),%rdi
mov    0x88(%rsp),%r8
mov    0x30(%rsp),%r10
mov    0x48(%rsp),%rbp
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e42>
mov    %r8,%r12
sub    %r15,%r12
mov    0x58(%rsp),%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e62>
lea    (%r11,%r15,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %r10,0x30(%rsp)
lea    0x0(,%r12,8),%rdx
mov    %rbp,0x48(%rsp)
call   *0x16bb31(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r15,%rbp
lea    (%r15,%r15,2),%r15
mov    0x8(%rsp),%rax
lea    (%rax,%r15,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r15,1),%rdi
add    $0x3,%rdi
lea    (%r12,%r12,2),%rdx
call   *0x16bb0a(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,%rsi
mov    0x8(%rsp),%rdi
mov    0x88(%rsp),%r8
mov    0x30(%rsp),%r10
mov    0x48(%rsp),%rbp
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e69>
mov    %r8,%r12
sub    %r15,%r12
mov    0x58(%rsp),%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e87>
lea    (%r11,%r15,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %r10,0x30(%rsp)
lea    0x0(,%r12,8),%rdx
mov    0x16bab0(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,0x48(%rsp)
mov    %rax,%rbp
call   *%rax
mov    0x8(%rsp),%rax
lea    (%rax,%r15,4),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r15,4),%rdi
add    $0x4,%rdi
shl    $0x2,%r12
mov    %r12,%rdx
call   *%rbp
lea    0x0(,%r15,4),%rsi
mov    0x8(%rsp),%rdi
mov    0x88(%rsp),%r8
mov    0x30(%rsp),%r10
mov    0x48(%rsp),%rbp
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e8f>
mov    %r8,%r12
sub    %r15,%r12
mov    0x58(%rsp),%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ea3>
lea    (%r11,%r15,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %r10,0x30(%rsp)
lea    0x0(,%r12,8),%rdx
mov    0x16ba28(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,0x48(%rsp)
mov    %rax,%rbp
call   *%rax
mov    0x8(%rsp),%rax
lea    (%rax,%r15,2),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r15,2),%rdi
add    $0x2,%rdi
add    %r12,%r12
mov    %r12,%rdx
call   *%rbp
lea    (%r15,%r15,1),%rsi
mov    0x8(%rsp),%rdi
mov    0x88(%rsp),%r8
mov    0x30(%rsp),%r10
mov    0x48(%rsp),%rbp
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ea7>
mov    %r8,%r12
sub    %r15,%r12
mov    0x58(%rsp),%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ebc>
lea    (%r11,%r15,8),%rsi
mov    %rdi,0x8(%rsp)
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %r10,0x30(%rsp)
lea    0x0(,%r12,8),%rdx
mov    %rbp,0x48(%rsp)
call   *0x16b9a1(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    (%r15,%r15,1),%rax
mov    %r15,%rbp
lea    (%rax,%rax,2),%r15
mov    0x8(%rsp),%rax
lea    (%rax,%r15,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r15,1),%rdi
add    $0x6,%rdi
add    %r12,%r12
lea    (%r12,%r12,2),%rdx
call   *0x16b973(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,%rsi
mov    0x8(%rsp),%rdi
mov    0x88(%rsp),%r8
mov    0x30(%rsp),%r10
mov    0x48(%rsp),%rbp
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ec7>
mov    %r8,%r14
sub    %r15,%r14
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ee8>
lea    (%r11,%r15,8),%rsi
lea    0x0(,%r15,8),%r12
mov    %rdi,0x8(%rsp)
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
lea    0x0(,%r14,8),%r15
mov    %r10,0x30(%rsp)
mov    %r15,%rdx
mov    0x16b913(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,0x48(%rsp)
mov    %rax,%rbp
call   *%rax
sub    0x38(%rsp),%r12
mov    0x8(%rsp),%rax
lea    (%rax,%r12,1),%rsi
mov    0x8(%rsp),%rax
lea    (%rax,%r12,1),%rdi
add    $0x7,%rdi
sub    %r14,%r15
mov    %r15,%rdx
mov    0x38(%rsp),%r15
call   *%rbp
mov    0x8(%rsp),%rdi
mov    0x88(%rsp),%r8
mov    0x30(%rsp),%r10
mov    0x48(%rsp),%rbp
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ef3>
mov    %r15d,0x60(%rsp)
shr    $0x20,%r15
mov    %r15b,0x64(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ae9>
mov    %r15w,0x60(%rsp)
shr    $0x10,%r15d
mov    %r15b,0x62(%rsp)
mov    %bp,0x63(%rsp)
shr    $0x10,%ebp
mov    %bpl,0x65(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ae9>
mov    %r15d,0x60(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ae9>
xor    %r15d,%r15d
mov    0x1c0(%rsp),%r11
mov    0x68(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ae9>
mov    %r15w,0x60(%rsp)
mov    %bp,0x62(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ae9>
mov    %r15d,0x60(%rsp)
shr    $0x20,%r15
mov    %r15w,0x64(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ae9>
mov    %r15d,0x60(%rsp)
mov    %r15,%rcx
shr    $0x20,%rcx
mov    %cx,0x64(%rsp)
shr    $0x30,%r15
mov    0x10(%rsp),%rsi
mov    %rax,(%rsi)
movzwl 0x64(%rsp),%ecx
mov    %cx,0x4(%rdi)
mov    0x60(%rsp),%ecx
mov    %ecx,(%rdi)
mov    %r15b,0xe(%rsi)
lea    0x1(%r13),%ecx
mov    %cl,0xf(%rsi)
test   %rdx,%rdx
mov    0x58(%rsp),%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b92>
shl    $0x4,%rdx
lea    (%rsp,%rdx,1),%rcx
add    $0x100,%rcx
lea    0x100(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b40>
mov    %sil,0x48(%r11,%rdi,1)
incq   0x50(%r11)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b92>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b72>
mov    0x8(%rdi),%r8
lea    -0x205cf0(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b37>
mov    %rdi,(%r11,%r8,8)
mov    0x50(%r11),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b2e>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5149>
lea    (%rax,%rbx,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x515f>
lea    (%r11,%r11,4),%rax
mov    %ebx,0x70(%rsp,%rax,1)
shr    $0x20,%rbx
mov    %bl,0x74(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c00>
lea    (%r11,%r11,2),%rax
mov    %bx,0x70(%rsp,%rax,1)
shr    $0x10,%ebx
mov    %bl,0x72(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c00>
mov    %ebx,0x70(%rsp,%r11,4)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c00>
mov    %bx,0x70(%rsp,%r11,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c00>
lea    (%r11,%r11,2),%rax
mov    %ebx,0x70(%rsp,%rax,2)
shr    $0x20,%rbx
mov    %bx,0x74(%rsp,%rax,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c00>
lea    (%rsp,%r11,8),%rax
add    $0x70,%rax
sub    %r11,%rax
mov    %ebx,(%rax)
mov    %rbx,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%rbx
mov    %bl,0x6(%rax)
imul   %r9d,%r14d
cmp    $0x10,%r14d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63a0>
mov    0x70(%rsp),%eax
mov    0x73(%rsp),%ecx
mov    %ecx,0x3(%r12)
mov    %eax,(%r12)
and    $0x70,%r13b
dec    %r9b
or     %r13b,%r9b
mov    0x10(%rsp),%rax
mov    %r9b,0xf(%rax)
mov    0x68(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ccd>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x100,%rax
lea    0x100(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c69>
nopw   0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ccd>
mov    (%rcx),%r11
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c9b>
mov    0x8(%r11),%rdi
lea    -0x205e19(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c60>
mov    0x1c0(%rsp),%r8
mov    %r11,(%r8,%rdi,8)
mov    0x50(%r8),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5149>
mov    %dl,0x48(%r8,%rdi,1)
incq   0x50(%r8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c60>
mov    0xa0(%rsp),%rax
movq   $0x0,(%rax)
mov    %rsi,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54da>
lea    (%r8,%r8,4),%rax
mov    %ebx,0x80(%rsp,%rax,1)
shr    $0x20,%rbx
mov    %bl,0x84(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d64>
lea    (%r8,%r8,2),%rax
mov    %bx,0x80(%rsp,%rax,1)
shr    $0x10,%ebx
mov    %bl,0x82(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d64>
mov    %ebx,0x80(%rsp,%r8,4)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d64>
mov    %bx,0x80(%rsp,%r8,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d64>
lea    (%r8,%r8,2),%rax
mov    %ebx,0x80(%rsp,%rax,2)
shr    $0x20,%rbx
mov    %bx,0x84(%rsp,%rax,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d64>
lea    (%rsp,%r8,8),%rax
add    $0x80,%rax
sub    %r8,%rax
mov    %ebx,(%rax)
mov    %rbx,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%rbx
mov    %bl,0x6(%rax)
imul   %edi,%r14d
cmp    $0x10,%r14d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63d2>
mov    0x10(%rsp),%rdx
mov    %r9,(%rdx)
mov    0x80(%rsp),%eax
mov    0x83(%rsp),%ecx
mov    %ecx,0x3(%rsi)
mov    %eax,(%rsi)
and    $0x70,%r13b
dec    %dil
or     %r13b,%dil
mov    %dil,0xf(%rdx)
mov    0x68(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e23>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x100,%rax
lea    0x100(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5dcf>
mov    %dl,0x48(%r12,%rdi,1)
incq   0x50(%r12)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e23>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e01>
mov    0x8(%rsi),%rdi
lea    -0x205f7f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5dc6>
mov    %rsi,(%r12,%rdi,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5dbc>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5149>
mov    0xa0(%rsp),%rax
movq   $0x0,(%rax)
mov    %r10,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54da>
mov    %r15,%rsi
lea    (%r15,%r15,4),%r15
mov    0x10(%rsp),%rcx
mov    0x40(%rsp),%rax
mov    %rax,(%r11,%rsi,8)
mov    %r10d,(%rdi,%r15,1)
shr    $0x20,%r10
mov    %r10b,0x4(%rdi,%r15,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ee3>
mov    %r15,%rsi
lea    (%r15,%r15,2),%r15
mov    0x10(%rsp),%rcx
mov    0x40(%rsp),%rax
mov    %rax,(%r11,%rsi,8)
mov    %r10w,(%rdi,%r15,1)
shr    $0x10,%r10d
mov    %r10b,0x2(%rdi,%r15,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ee3>
lea    0x0(,%r15,4),%rsi
mov    0x10(%rsp),%rcx
mov    0x40(%rsp),%rax
mov    %rax,(%r11,%r15,8)
mov    %r10d,(%rdi,%rsi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f20>
lea    (%r15,%r15,1),%rsi
mov    0x10(%rsp),%rcx
mov    0x40(%rsp),%rax
mov    %rax,(%r11,%r15,8)
mov    %r10w,(%rdi,%rsi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f20>
lea    (%r15,%r15,1),%rax
mov    %r15,%rsi
lea    (%rax,%rax,2),%r15
mov    0x10(%rsp),%rcx
mov    0x40(%rsp),%rax
mov    %rax,(%r11,%rsi,8)
mov    %r10d,(%rdi,%r15,1)
shr    $0x20,%r10
mov    %r10w,0x4(%rdi,%r15,1)
mov    %rsi,%r15
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f20>
lea    0x0(,%r15,8),%r12
sub    %r15,%r12
mov    0x58(%rsp),%r14
mov    0x10(%rsp),%rcx
mov    0x40(%rsp),%rax
mov    %rax,(%r11,%r15,8)
mov    %r10d,(%rdi,%r12,1)
mov    %r10,%rax
shr    $0x20,%rax
mov    %ax,0x4(%rdi,%r12,1)
shr    $0x30,%r10
mov    %r10b,0x6(%rdi,%r12,1)
mov    0x50(%rsp),%rdx
mov    (%rdx),%rax
xor    %rax,%r8
and    %rbp,%r8
xor    %rax,%r8
mov    %r8,(%rdx)
cmp    $0x1,%r13d
mov    0x1c0(%rsp),%r10
mov    0x28(%rsp),%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f93>
cmp    $0x1,%al
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f93>
shr    $0x8,%rbx
mov    %rbx,0x40(%r10)
movq   $0x0,0x58(%r10)
mov    %r11,0x60(%r10)
mov    0xf0(%rsp),%rax
mov    %ax,0x70(%r10)
mov    %rcx,(%r10)
movb   $0x1,0x48(%r10)
movq   $0x1,0x50(%r10)
movq   $0x0,0x68(%r10)
mov    $0x1,%edx
mov    0x68(%rsp),%r9
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61af>
mov    (%r14),%rax
test   %rax,%rax
mov    0x68(%rsp),%r9
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61ad>
mov    0x68(%r10),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x618d>
movq   $0x0,0x68(%r10)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x618d>
movzbl 0x49(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ff7>
mov    0x8(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x206175(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x618d>
movzbl 0x4a(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6035>
mov    0x10(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2061b3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x618d>
movzbl 0x4b(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6073>
mov    0x18(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2061f1(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x618d>
movzbl 0x4c(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x60b1>
mov    0x20(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20622f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x618d>
movzbl 0x4d(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x60ef>
mov    0x28(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x20626d(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x618d>
movzbl 0x4e(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x612d>
mov    0x30(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2062ab(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x618d>
movzbl 0x4f(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x616f>
mov    0x1c0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2062ed(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
mov    0x58(%rsp),%r14
mov    0x68(%rsp),%r9
mov    0x38(%rsp),%r15
mov    0x18(%rsp),%r11
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x656f>
movq   $0xffffffffffffffff,0x40(%r10)
movw   $0x0,0x70(%r10)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r14)
movq   $0x0,0x10(%r14)
xor    %edx,%edx
test   %r9,%r9
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x623b>
shl    $0x4,%r9
lea    (%rsp,%r9,1),%rax
add    $0x100,%rax
lea    0x100(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61e9>
mov    %sil,0x48(%r10,%rdi,1)
mov    0x50(%r10),%rdx
inc    %rdx
mov    %rdx,0x50(%r10)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x623b>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x621e>
mov    0x8(%rdi),%rdx
lea    -0x206399(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rsi,8),%r8
mov    %r8,%r9
and    %rdx,%r9
inc    %r9
xor    %rdx,%r9
and    %r8,%r9
xor    %rdx,%r9
mov    %r9,0x8(%rdi)
mov    (%r14),%rdx
lea    -0x1(%rdx),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61e0>
mov    %rdi,(%r10,%rdx,8)
mov    0x50(%r10),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61d0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5149>
lea    (%r11,%r15,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x515f>
lea    -0x20639d(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8db>
lea    0x161c2e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x910>
mov    $0xa1,%esi
call   *0x16b053(%rip)        # ADDR <_DYNAMIC+0x278>
lea    0x161e44(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xb38>
mov    $0x7,%esi
call   *0x16afd1(%rip)        # ADDR <_DYNAMIC+0x208>
lea    -0x20633b(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc968>
lea    0x161c1b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x928>
mov    $0x13,%esi
call   *0x16b180(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x229a87(%rip),%rdi        # ADDR <anon.ADDR.91.llvm.ADDR+0x260>
lea    0x161b72(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x898>
mov    $0x8,%esi
call   *0x16b167(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x229a68(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
lea    0x161d09(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa48>
mov    $0x8,%esi
call   *0x16b14e(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x161c7f(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x9d0>
mov    0x38(%rsp),%rdi
mov    %rbp,%rsi
call   *0x16b009(%rip)        # ADDR <_DYNAMIC+0x2a0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65bb>
lea    0x161cc5(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa30>
mov    $0x8,%esi
lea    -0x229aa7(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62fc>
lea    0x161c98(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa18>
mov    $0x11,%esi
lea    -0x2063d2(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc957>
call   *0x16b106(%rip)        # ADDR <_DYNAMIC+0x3d0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65bb>
lea    -0x206466(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8d5>
lea    0x161b53(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8f8>
mov    $0x6,%esi
call   *0x16b0e8(%rip)        # ADDR <_DYNAMIC+0x3d0>
call   *0x16b202(%rip)        # ADDR <_DYNAMIC+0x4f0>
test   %al,%al
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5473>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5493>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0x16af75(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x160466(%rip),%rcx        # ADDR <anon.ADDR.892.llvm.ADDR>
mov    %rdx,%rsi
mov    $0x7,%edx
mov    %r15,%rdi
call   *0x16af2d(%rip)        # ADDR <_DYNAMIC+0x250>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65bb>
lea    -0x206457(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc93d>
lea    0x161b42(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x940>
mov    $0xb,%esi
call   *0x16b08f(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x1615f0(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
call   *0x16aeb5(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1615de(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
mov    %rax,%rdi
call   *0x16aea0(%rip)        # ADDR <_DYNAMIC+0x208>
lea    -0x2064a9(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x161b7a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x9b8>
mov    $0x12,%esi
call   *0x16b04f(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x2064b0(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc93d>
lea    0x161b31(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x988>
mov    $0xb,%esi
call   *0x16b036(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x2064db(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x161b00(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x970>
mov    $0x12,%esi
call   *0x16b01d(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x1615de(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x460>
mov    $0x8,%esi
call   *0x16ae43(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161a4c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8e0>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16ae2e(%rip)        # ADDR <_DYNAMIC+0x208>
lea    -0x206644(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0x1681f0(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x161379(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0xcf(%rsp),%rdx
mov    $0x11,%esi
call   *0x16aec6(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65bb>
lea    0x1619f2(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8c8>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16adec(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0x16ae54(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65bb>
lea    0x161908(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x808>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16adc2(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161923(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x838>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16adad(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161926(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x850>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0x16ad96(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1618df(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x820>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16ad81(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161912(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x868>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16ad6c(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161915(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x880>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0x16ad55(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x8,%esi
call   *0x16ad4a(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65bb>
lea    0x16191e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8b0>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16ad30(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1612a1(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %rbp,%rdi
call   *0x16ad1b(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x16125c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
call   *0x16ad09(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x16127a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r15,%rdi
call   *0x16acf4(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161265(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r14,%rdi
call   *0x16acdf(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x161220(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64f0>
lea    0x16834a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0x16acba(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1619cb(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x9e8>
xor    %edi,%edi
xor    %esi,%esi
call   *0x16aca9(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65bb>
mov    %r13,%rdi
lea    0x1614a5(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4d8>
mov    $0x8,%esi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65b5>
lea    0x1619bf(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa00>
xor    %edi,%edi
xor    %esi,%esi
call   *0x16ac85(%rip)        # ADDR <_DYNAMIC+0x208>
ud2
mov    $0x10,%edi
mov    0x30(%rsp),%rsi
call   *0x16aceb(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %r11,%r15
call   *0x16af62(%rip)        # ADDR <_DYNAMIC+0x500>
mov    %r15,%r11
cmp    $0xf,%r15
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4121>
movq   $0x0,0xa8(%rsp)
lea    0xa8(%rsp),%rdi
mov    $0x10,%esi
mov    %r11,%rdx
mov    %r11,0x20(%rsp)
call   *0x16ad26(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
mov    0x38(%rsp),%r15
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66b4>
mov    0xa8(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66b4>
mov    %r13,%rbp
xor    %esi,%esi
mov    0x20(%rsp),%rdx
mov    %rdi,0x20(%rsp)
call   *0x16ad03(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x20(%rsp),%r10
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4148>
mov    %rbp,%rsi
mov    %rdx,%r14
call   *0x16aee5(%rip)        # ADDR <_DYNAMIC+0x500>
mov    %r14,%rdx
mov    $0x10,%esi
mov    %rdx,%r14
mov    %rdx,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%r15
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x320b>
mov    $0x10,%edi
mov    %r14,%rsi
call   *0x16ac38(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %rcx,%rdi
call   *0x16aee7(%rip)        # ADDR <_DYNAMIC+0x538>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x544c>
mov    %rcx,%rdi
call   *0x16aed9(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %r14,%rdi
call   *0x16abc8(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x50(%rsp),%r9
mov    0x38(%rsp),%r8
mov    0x20(%rsp),%r13
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3306>
mov    $0x10,%edi
mov    0x20(%rsp),%rsi
call   *0x16abf4(%rip)        # ADDR <_DYNAMIC+0x280>
mov    0x8(%rsp),%rdi
mov    %r13,%rsi
mov    %rdx,%r15
call   *0x16ae63(%rip)        # ADDR <_DYNAMIC+0x500>
mov    %r15,%rdx
mov    $0x10,%esi
mov    %rdx,%r15
mov    %rdx,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%r11
test   %rax,%rax
mov    0x58(%rsp),%rbp
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x35ab>
mov    $0x10,%edi
mov    %r15,%rsi
call   *0x16abb1(%rip)        # ADDR <_DYNAMIC+0x280>
mov    0x8(%rsp),%rdi
mov    %r11,%r15
call   *0x16ae5b(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %rbx,%rdi
call   *0x16ab4a(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %r15,%r11
mov    0x90(%rsp),%r10
mov    0x18(%rsp),%r8
mov    0x10(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5644>
mov    0x8(%rsp),%rdi
mov    %rdx,%rsi
mov    %rdx,%rbp
call   *0x16adef(%rip)        # ADDR <_DYNAMIC+0x500>
mov    $0x10,%esi
mov    %rbp,0x48(%rsp)
mov    %rbp,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,0x30(%rsp)
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47c4>
mov    $0x10,%edi
mov    0x48(%rsp),%rsi
call   *0x16ab3f(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %rcx,%rdi
mov    %r10,0x28(%rsp)
call   *0x16ade9(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %rbp,%rdi
call   *0x16aad8(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x28(%rsp),%r10
mov    0x30(%rsp),%r9
mov    0x90(%rsp),%r8
mov    0x18(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x48bb>
mov    %rax,%rbx
cmpq   $0x0,0xa8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x67cf>
mov    0xb0(%rsp),%rdi
call   *0x16aaa0(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x67cf>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x67cc>
mov    %rax,%rbx
cmpq   $0x0,0xd8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x67e8>
mov    0xe0(%rsp),%rdi
call   *0x16aa80(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
