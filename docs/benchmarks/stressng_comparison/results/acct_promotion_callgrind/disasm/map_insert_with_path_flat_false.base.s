push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x188,%rsp
mov    %r9,0x28(%rsp)
mov    %r8,0x50(%rsp)
mov    %rcx,%rbx
mov    %rsi,0x20(%rsp)
mov    %rdi,0x98(%rsp)
mov    0x1c0(%rsp),%r12
lea    0x50(%r12),%rbp
xor    %r14d,%r14d
mov    %rbp,0x40(%rsp)
mov    %rcx,0xa0(%rsp)
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1103>
lea    -0x203caf(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x1839>
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
lea    -0x1f8ca6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
lea    -0x1f8ce6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
lea    -0x1f8d26(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
lea    -0x1f8d66(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
lea    -0x1f8da6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
lea    -0x1f8de2(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
lea    -0x1f8e1e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
cmpb   $0x8,0x28(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29da>
mov    %r14,%r15
mov    0x20(%rsp),%rdx
lock addq $0x40,0x200(%rdx)
lock incq 0x208(%rdx)
lock incq 0x218(%rdx)
mov    0x18(%rdx),%r14
test   %r14,%r14
mov    0x10(%rsp),%rcx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a>
movq   $0x0,0xa8(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa8(%rsp),%rdi
call   *0x172202(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x645b>
mov    0xa8(%rsp),%r14
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x645b>
mov    $0x1000,%edx
mov    %r14,%rdi
xor    %esi,%esi
call   *0x1721e9(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x20(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%r14)
movq   $0x1,0x8(%r14)
mov    %r14,0x220(%rdi)
lea    0xfc0(%r14),%rax
mov    $0x41,%ecx
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
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
jg     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3b0>
add    $0x40,%r14
mov    0x10(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x51>
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6533>
mov    (%rdx),%rax
mov    %r14,%rcx
shl    $0x4,%rcx
mov    %rdx,0x108(%rsp,%rcx,1)
mov    0x28(%rsp),%rdx
mov    %dl,0x110(%rsp,%rcx,1)
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
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x25c7>
mov    0x28(%rsp),%rax
dec    %eax
movzbl %al,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64a1>
lea    -0x1f8fd7(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rcx
shl    $0x8,%rcx
mov    0x10(%rsp),%rax
mov    0x8(%rax),%rax
mov    %rax,%rdx
xor    %rbx,%rdx
test   %rcx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x25c7>
mov    0x0(%rbp),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x687>
mov    0x68(%r12),%rdx
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x665>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x665>
movzbl 0x49(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e3>
mov    0x8(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x1f9041(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x2,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x665>
movzbl 0x4a(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x523>
mov    0x10(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x1f9081(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x3,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x665>
movzbl 0x4b(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x563>
mov    0x18(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x1f90c1(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x4,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x665>
movzbl 0x4c(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a3>
mov    0x20(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x1f9101(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x5,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x665>
movzbl 0x4d(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e3>
mov    0x28(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x1f9141(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x6,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x665>
movzbl 0x4e(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61f>
mov    0x30(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x1f917d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x7,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x665>
movzbl 0x4f(%r12),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65b>
mov    0x38(%r12),%rdi
mov    0x8(%rdi),%r8
lea    -0x1f91b9(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rdx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x8,%rcx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x635b>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6d0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1404>
mov    (%rdx),%r13
movzbl 0x74(%r13),%r10d
movzbl %r10b,%edi
mov    0x28(%rsp),%rax
sub    %dil,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf89>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64b3>
lea    -0x1f929d(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x10(%rsp),%rdx
mov    0x8(%rdx),%r8
mov    %r8,%rcx
xor    %rbx,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf89>
cmp    $0x7,%r10b
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6521>
mov    %rdx,%rcx
lea    -0x1f92d7(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x970>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x949>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x949>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x7c7>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9325(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x949>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x807>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9365(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x949>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x847>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f93a5(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x949>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x887>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f93e5(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x949>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x8c7>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9425(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x949>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x903>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9461(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x949>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x93f>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f949d(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x635b>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x9b0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1404>
mov    (%rdx),%rdx
movzbl 0x5(%rdx),%esi
mov    0x28(%rsp),%rax
sub    %sil,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1040>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64b3>
movzbl %sil,%edi
lea    -0x1f957c(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x10(%rsp),%r9
mov    0x8(%r9),%r8
mov    %r8,%rcx
xor    %rbx,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1040>
cmp    $0x7,%sil
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6521>
mov    %r9,%rcx
lea    -0x1f95b6(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc4f>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc28>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc28>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xaa6>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9604(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc28>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xae6>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9644(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc28>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xb26>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9684(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc28>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xb66>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f96c4(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc28>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xba6>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9704(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc28>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xbe2>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9740(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc28>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc1e>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f977c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x635b>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc80>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1404>
mov    (%rdx),%r15
movzbl 0x5(%r15),%ebp
mov    0x28(%rsp),%rax
sub    %bpl,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x10ab>
movzbl %al,%eax
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64b3>
movzbl %bpl,%edi
lea    -0x1f984d(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rax,8),%rax
lea    0x0(,%rdi,8),%ecx
shl    %cl,%rax
mov    0x10(%rsp),%rdx
mov    0x8(%rdx),%r8
mov    %r8,%rcx
xor    %rbx,%rcx
test   %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x10ab>
cmp    $0x7,%bpl
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6521>
mov    %rdx,%rcx
lea    -0x1f9887(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rax,%rdi,8),%r8
mov    0x40(%rsp),%rbp
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf25>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xefe>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xefe>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xd7c>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f98da(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xefe>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xdbc>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f991a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xefe>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xdfc>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f995a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xefe>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xe3c>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f999a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xefe>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xe7c>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f99da(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xefe>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xeb8>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9a16(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xefe>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xef4>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9a52(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x635b>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xf60>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1404>
mov    %r14,0x78(%rsp)
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
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1423>
mov    0x78(%rsp),%r14
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6548>
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
mov    %rdx,0x108(%rsp,%rcx,1)
mov    %r10b,0x110(%rsp,%rcx,1)
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
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
movzbl 0x8(%rdx),%r8d
cmp    %al,%r8b
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1731>
xor    %r9d,%r9d
cmp    $0x7,%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65b3>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rcx
mov    %rcx,0x108(%rsp,%rax,1)
mov    %sil,0x110(%rsp,%rax,1)
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
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x17b6>
cmp    $0x4,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1773>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cd3>
cmp    %dl,0x8(%r15)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cd3>
xor    %ecx,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cea>
cmp    $0x7f,%r13d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6342>
lea    -0x5(%r13),%eax
cmp    $0x7,%al
mov    %r14,0x78(%rsp)
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x265f>
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x132b>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1304>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1304>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1182>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9ce0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1304>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x11c2>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9d20(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1304>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1202>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9d60(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1304>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1242>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9da0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1304>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1282>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9de0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1304>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12be>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9e1c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1304>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x12fa>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1f9e58(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x10(%rsp),%rdx
lea    -0x4(%r13),%eax
lea    0x8(%rdx),%rcx
mov    %rcx,0x58(%rsp)
movzbl %al,%r14d
mov    0x8(%rdx),%rax
lea    -0x1f9ebf(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%r14,8),%r10
mov    %r10,%rbp
and    %rax,%rbp
lea    0x1(%rbp),%r8
mov    0x28(%rsp),%rcx
cmp    %r14b,%cl
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29ea>
sub    %r14b,%cl
movzbl %cl,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64a1>
lea    -0x1f9ef2(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rcx,%rdi,8),%rdx
lea    0x0(,%r14,8),%r15d
mov    %r15d,%ecx
shl    %cl,%rdx
xor    %rbx,%rax
test   %rax,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x29f2>
mov    0x58(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0xab(%rsp)
mov    %eax,0xa8(%rsp)
mov    0x28(%rsp),%rcx
movzbl %cl,%eax
lea    0x1(%rcx),%edx
lea    -0x8(,%rax,8),%rcx
lea    -0x1(%rax),%rdi
mov    0x40(%rsp),%rbp
mov    0x78(%rsp),%r14
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    $0x6,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x635b>
mov    %rbx,%rsi
shr    %cl,%rsi
dec    %dl
add    $0xfffffffffffffff8,%rcx
cmp    %sil,0xa8(%rsp,%rdi,1)
lea    -0x1(%rdi),%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x13e0>
movzbl %dl,%edx
mov    0x20(%rsp),%rdi
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
movdqa -0x21e910(%rip),%xmm3        # ADDR <anon.ADDR.407.llvm.ADDR+0x10>
pand   %xmm3,%xmm2
psubb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
movdqa -0x21e674(%rip),%xmm4        # ADDR <anon.ADDR.407.llvm.ADDR+0x2c0>
pand   %xmm4,%xmm2
psrlw  $0x2,%xmm1
pand   %xmm4,%xmm1
paddb  %xmm2,%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x4,%xmm2
paddb  %xmm1,%xmm2
movdqa -0x21e9ea(%rip),%xmm5        # ADDR <anon.ADDR.283.llvm.ADDR+0xf6>
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
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19ee>
cmp    %r10b,0x28(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d7b>
cmp    $0x8,%r10b
mov    0x78(%rsp),%r14
mov    0x10(%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6521>
lea    -0x1fa078(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%r8
and    0x8(%rcx),%r8
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1710>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ee>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ee>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x156c>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa0ca(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ee>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x15ac>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa10a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ee>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x15ec>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa14a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ee>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x162c>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa18a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ee>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x166c>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa1ca(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ee>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16a8>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa206(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16ee>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x16e4>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa242(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rdx,8),%rdx
mov    %rdx,%r9
and    %rdi,%r9
add    %rcx,%r9
xor    %rdi,%r9
and    %rdx,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
inc    %r8
inc    %r10b
movzbl 0x28(%rsp),%ecx
movzbl %r10b,%edx
mov    0x20(%rsp),%rdi
mov    0x10(%rsp),%r15
mov    %r15,%rsi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1416>
cmp    $0x1,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1a81>
cmp    %al,%r8b
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d40>
lea    0x10(%rdx),%r9
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2389>
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2397>
movzbl 0x9(%rdx),%r10d
mov    %r10b,0xa(%rdx)
mov    %r8b,0x9(%rdx)
movups 0x20(%rdx),%xmm0
movups %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x238d>
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
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x17b6>
tzcnt  %esi,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cea>
cmp    $0x7,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1fe5>
mov    0x40(%rsp),%rbp
mov    0x0(%rbp),%rax
test   %rax,%rax
mov    0x1c0(%rsp),%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19d4>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19b2>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19b2>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1830>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa38e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19b2>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1870>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa3ce(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19b2>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x18b0>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa40e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19b2>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x18f0>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa44e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19b2>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1930>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa48e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19b2>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x196c>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa4ca(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19b2>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x19a8>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa506(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x20(%rsp),%rdi
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
mov    %r10,0x58(%rsp)
mov    %rsi,0x38(%rsp)
mov    %r8,0x18(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2068>
mov    $0x2,%r12d
cmp    $0x1,%r8d
mov    0x20(%rsp),%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2073>
cmp    $0x10,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x23ff>
cmp    $0x2,%r8w
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x24fe>
lea    0x4(%r8),%r12
and    $0x1c,%r12d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2073>
mov    $0x1,%r9d
cmp    %al,0x9(%rdx)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x106d>
cmp    $0x2,%cl
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x173a>
mov    $0x2,%r9d
cmp    %al,0xa(%rdx)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x106d>
cmp    $0x3,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x173a>
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1cb9>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c97>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c97>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1b15>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa673(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c97>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1b55>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa6b3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c97>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1b95>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa6f3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c97>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1bd5>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa733(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c97>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c15>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa773(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c97>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c51>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa7af(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c97>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1c8d>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa7eb(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x20(%rsp),%rdi
mov    0x10(%rsp),%r15
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate::upgrade_l3_to_l7::<false>>
mov    %r15,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
cmp    $0x2,%al
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1fa2>
mov    $0x1,%ecx
cmp    %dl,0x9(%r15)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1fa2>
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6587>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rdx
mov    %rdx,0x108(%rsp,%rax,1)
mov    %bpl,0x110(%rsp,%rax,1)
cmp    $0x7,%ecx
mov    0x1c0(%rsp),%r12
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x659c>
inc    %r14
shl    $0x4,%ecx
lea    (%r15,%rcx,1),%rdx
add    $0x10,%rdx
mov    %edi,%eax
mov    %rax,0x28(%rsp)
mov    0x40(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
movzbl 0x9(%rdx),%r9d
cmp    $0x1,%ecx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d59>
mov    $0x2,%r8d
cmp    %al,%r9b
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x239a>
mov    $0x1,%r8d
cmp    $0x2,%ecx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x239a>
mov    %r9b,0xa(%rdx)
movdqu 0x20(%rdx),%xmm0
movdqu %xmm0,0x30(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x239a>
mov    0x0(%rbp),%rax
test   %rax,%rax
mov    0x78(%rsp),%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f88>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f66>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f66>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1de4>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa942(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f66>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1e24>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa982(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f66>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1e64>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fa9c2(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f66>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1ea4>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1faa02(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f66>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1ee4>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1faa42(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f66>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f20>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1faa7e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f66>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1f5c>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1faaba(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
mov    0x20(%rsp),%rdi
mov    0x10(%rsp),%r15
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate::upgrade_b_to_u::<false>>
mov    %r15,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
cmp    $0x3,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1fe5>
cmp    %dl,0xa(%r15)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1fe5>
cmp    $0x7,%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6587>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rcx
mov    %rcx,0x108(%rsp,%rax,1)
mov    %bpl,0x110(%rsp,%rax,1)
mov    $0x2,%ecx
mov    0x1c0(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x1d21>
mov    %rdi,%rbx
lea    0x8(%r15),%rdi
lea    0x10(%r15),%r13
movzbl %dl,%r8d
mov    $0x7,%edx
mov    %r13,%rsi
mov    %rax,%rcx
call   *0x1707f3(%rip)        # ADDR <_DYNAMIC+0x5a0>
incb   0x4(%r15)
or     %r12w,0x6(%r15)
cmp    $0x8,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65c8>
mov    %r14,%rcx
shl    $0x4,%rcx
mov    0x10(%rsp),%rdx
mov    %rdx,0x108(%rsp,%rcx,1)
mov    %bpl,0x110(%rsp,%rcx,1)
cmp    $0x7,%eax
mov    0x1c0(%rsp),%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65dd>
inc    %r14
shl    $0x4,%eax
add    %rax,%r13
mov    %rbx,0x28(%rsp)
mov    %r13,%rdx
mov    0x40(%rsp),%rbp
mov    0xa0(%rsp),%rbx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
mov    $0x1,%r12d
mov    0x20(%rsp),%rcx
mov    %r12d,%edx
shl    $0x4,%edx
lock add %rdx,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
lea    -0x1f8aef(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rdx,%rax,1),%ebp
cmp    $0xff,%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2268>
cmp    $0x3d,%bpl
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6619>
mov    0x10(%rcx,%rbp,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x20ca>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2253>
cmp    $0x11,%r12d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2268>
mov    %rdx,0x60(%rsp)
movq   $0x0,0xa8(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa8(%rsp),%rdi
call   *0x170453(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x645b>
mov    0xa8(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x645b>
mov    $0x1000,%edx
xor    %esi,%esi
mov    %rdi,0x30(%rsp)
call   *0x170438(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x30(%rsp),%r9
mov    0x20(%rsp),%r11
mov    0x220(%r11),%rax
mov    %rax,(%r9)
mov    %rbp,0x8(%r9)
mov    %r9,0x220(%r11)
mov    $0xfc0,%ax
xor    %edx,%edx
mov    0x60(%rsp),%r10
div    %r10w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2196>
mov    %rcx,%rax
nopl   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rsi
imul   %r10,%rsi
lea    (%r9,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%r11,%rbp,8),%r8
mov    %r8,0x40(%r9,%rsi,1)
mov    %rdi,0x10(%r11,%rbp,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2170>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2245>
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
mov    0x30(%rsp),%r8
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x21f0>
mov    0x30(%rsp),%rdi
add    $0x40,%rdi
mov    0x60(%rsp),%rdx
xor    %esi,%esi
mov    %rdi,%rbp
call   *0x170302(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %rbp,%rdi
mov    0x40(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x228a>
mov    0x40(%rsp),%r12
mov    $0x1,%esi
mov    %rdx,%rdi
mov    %rdx,%rbp
call   *0x1702da(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%rdi
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4712>
mov    0x18(%rsp),%r8
test   %r8,%r8
mov    0x58(%rsp),%rdx
mov    0x38(%rsp),%rsi
mov    %r12,%rbp
mov    0x1c0(%rsp),%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2481>
mov    %r13,0x48(%rsp)
mov    0x20(%r13,%r15,8),%rsi
mov    %rsi,0x60(%rsp)
mov    0x28(%rsp),%rbp
shl    $0x4,%ebp
mov    %rbp,%rdx
mov    %r8,%r12
mov    %rdi,%r13
call   *0x17019d(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,0x30(%rsp)
lea    0x10(,%rbp,1),%rdi
add    %r13,%rdi
mov    0x60(%rsp),%r13
add    %r13,%rbp
mov    %r12,%rdx
sub    0x28(%rsp),%rdx
shl    $0x4,%rdx
mov    %rbp,%rsi
call   *0x170170(%rip)        # ADDR <memcpy@GLIBC_2.14>
test   %r13,%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6386>
mov    %r13,%rsi
mov    %r12,%rax
cmp    $0x3,%r12w
mov    0x20(%rsp),%rcx
mov    %r12,%r8
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2330>
cmp    $0x11,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x241e>
lea    0x3(%r8),%eax
and    $0x3c,%eax
shl    $0x4,%eax
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
lea    -0x1f8da1(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rax,%rdx,1),%edi
cmp    $0xff,%rdi
mov    0x40(%rsp),%rbp
mov    0x1c0(%rsp),%r12
mov    0x48(%rsp),%r13
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2464>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x662e>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rsi)
mov    %rsi,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2472>
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
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6572>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rcx
mov    %rcx,0x108(%rsp,%rax,1)
add    %r8,%rdx
add    $0x10,%rdx
mov    %sil,0x110(%rsp,%rax,1)
inc    %r14
mov    %rdi,0x28(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
cmp    $0x18,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2517>
mov    $0x180,%edx
cmp    $0x10,%r8d
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2545>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x25a7>
mov    $0x180,%eax
cmp    $0x19,%r8w
mov    0x40(%rsp),%rbp
mov    0x1c0(%rsp),%r12
mov    0x48(%rsp),%r13
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2454>
mov    $0x200,%eax
cmp    $0x21,%r8w
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2454>
mov    %r8d,%eax
shl    $0x4,%eax
add    $0x30,%eax
and    $0xffffffc0,%eax
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
mov    %rsi,%rdi
call   *0x17001b(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x18(%rsp),%r8
mov    0x58(%rsp),%rdx
mov    0x38(%rsp),%rsi
mov    0x30(%rsp),%rdi
mov    0x28(%rsp),%rax
shl    $0x4,%eax
pxor   %xmm0,%xmm0
movdqu %xmm0,(%rdi,%rax,1)
mov    %rdi,0x20(%r13,%r15,8)
mov    $0x1,%eax
mov    %r14d,%ecx
shl    %cl,%rax
inc    %r8d
mov    %r8w,0x60(%r13,%r15,2)
shr    $0x3,%r14d
and    $0x18,%r14d
or     %rax,0x0(%r13,%r14,1)
mov    0x78(%rsp),%r14
cmp    $0x7,%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6604>
mov    %r14,%rax
shl    $0x4,%rax
mov    0x10(%rsp),%rcx
mov    %rcx,0x108(%rsp,%rax,1)
mov    %dl,0x110(%rsp,%rax,1)
inc    %r14
mov    0x28(%rsp),%rdx
shl    $0x4,%edx
add    0x20(%r13,%r15,8),%rdx
mov    %esi,%eax
mov    %rax,0x28(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0xc0>
lea    0x4(%r8),%r12
lea    0x3(%r8),%eax
xor    %r12d,%eax
cmp    $0x4,%eax
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2545>
and    $0x1c,%r12d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2073>
cmp    $0x20,%r8w
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x252b>
mov    $0x200,%edx
cmp    $0x18,%r8d
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2545>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x25a7>
cmp    $0x20,%r8d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x259b>
lea    0x4(%r8),%eax
lea    0x3(%r8),%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
mov    0x20(%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x259b>
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
call   *0x16ffd0(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x18(%rsp),%r8
mov    0x38(%rsp),%rsi
mov    0x58(%rsp),%rdx
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12,%rbp,1)
mov    0x40(%rsp),%rbp
mov    0x1c0(%rsp),%r12
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2497>
mov    %r8d,%edx
shl    $0x4,%edx
add    $0x40,%edx
and    $0xffffffc0,%edx
mov    %rbp,%r12
lock add %rdx,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x226d>
mov    %r14,0x78(%rsp)
mov    %ebx,%r14d
shr    $0x5,%r14d
and    $0x7,%r14d
mov    $0x1,%eax
mov    %ebx,%ecx
shl    %cl,%eax
mov    0x10(%rsp),%rcx
mov    (%rcx),%rsi
mov    (%rsi,%r14,4),%r13d
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
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2a8d>
mov    0x20(%rsi,%r14,8),%rax
lea    (%rax,%r8,8),%rcx
mov    (%rax,%r8,8),%rdx
mov    0x50(%rsp),%rsi
mov    %rsi,(%rax,%r8,8)
mov    0x98(%rsp),%rax
movq   $0x1,(%rax)
mov    %rdx,0x8(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c1c>
test   %r13b,%r13b
js     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x636d>
mov    %r13d,%r14d
shr    $0x4,%r14d
test   %r14b,%r14b
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x636d>
mov    %r13d,%r10d
and    $0xf,%r10d
lea    0x1(%r10),%rsi
mov    %rsi,%r15
imul   %r14,%r15
cmp    $0xf,%r15
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x636d>
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x289c>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x287a>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x287a>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x26f8>
mov    0x8(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x1fb256(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x287a>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2738>
mov    0x10(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x1fb296(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x287a>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2778>
mov    0x18(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x1fb2d6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x287a>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27b8>
mov    0x20(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x1fb316(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x287a>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x27f8>
mov    0x28(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x1fb356(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x287a>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2834>
mov    0x30(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x1fb392(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x287a>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2870>
mov    0x38(%r12),%r9
mov    0x8(%r9),%rdi
lea    -0x1fb3ce(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%r9)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
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
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c76>
mov    %r10,0x28(%rsp)
movzbl %r14b,%edx
mov    %rdi,0xa0(%rsp)
mov    %rdx,0x38(%rsp)
mov    %rbx,%rcx
mov    %rsi,%rbp
call   ADDR <expanse_trie::leaf::locate>
mov    %rdx,%r11
mov    0x10(%rsp),%rcx
mov    (%rcx),%r8
test   $0x1,%al
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2cad>
mov    $0x7,%al
movzbl %al,%eax
div    %r14b
lea    0x4(%rbp),%rcx
cmp    %bpl,%al
mov    %rbp,0x30(%rsp)
mov    %r11,0x58(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2e71>
mov    %rbp,%rdx
lea    0x1(%rbp),%r9
cmpb   $0x1,0x28(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3018>
lea    0x3(%rdx),%rax
xor    %rax,%rcx
cmp    $0x4,%ecx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3018>
lea    (%r8,%r11,8),%rsi
cmp    %r11,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3cd9>
lea    0x1(%r11),%rbp
mov    %r9,0x70(%rsp)
lea    (%r8,%r11,8),%rdi
add    $0x8,%rdi
sub    %r11d,%edx
shl    $0x3,%edx
mov    %r8,%r12
mov    %rsi,0x28(%rsp)
call   *0x16fbd3(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x50(%rsp),%rax
mov    0x58(%rsp),%rcx
mov    %rax,(%r12,%rcx,8)
mov    0xa0(%rsp),%r12
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %eax,0x80(%rsp)
mov    %ecx,0x83(%rsp)
mov    0x58(%rsp),%rdx
imul   %r14,%rdx
imul   %r14,%rbp
lea    0x166782(%rip),%r9        # ADDR <anon.ADDR.2.llvm.ADDR+0x9a0>
lea    0x80(%rsp),%rdi
mov    $0x7,%esi
mov    %r15,%rcx
mov    %rbp,%r8
call   ADDR <<[u8]>::copy_within::<core::ops::range::Range<usize>>>
mov    0x28(%rsp),%rsi
mov    0x70(%rsp),%r9
mov    0x58(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d00>
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2b05>
mov    0x28(%rsp),%r10
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2b22>
lea    0x0(,%r14,8),%r15d
mov    $0xffffffffffffffff,%rdx
mov    %r15d,%ecx
shl    %cl,%rdx
not    %rdx
and    %rbx,%rdx
mov    0x10(%rsp),%rax
mov    (%rax),%r11
cmp    $0x3,%r8
mov    %r11,0x18(%rsp)
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2a73>
mov    %r8,%rax
test   %r8,%r8
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c3f>
cmp    $0x1,%r14
sete   %r9b
lea    0x8(%r14),%r13
mov    0x58(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x93(%rsp)
mov    %eax,0x90(%rsp)
mov    $0x1,%eax
mov    %rax,0xd8(%rsp)
movq   $0x0,0x60(%rsp)
xor    %esi,%esi
movq   $0x0,0x38(%rsp)
mov    0x20(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4087>
cmp    $0x11,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c25>
mov    %rbp,%rax
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
imul   $0x1010101,%eax,%r9d
shr    $0x18,%r9d
test   %r13d,%r13d
mov    %rsi,0x58(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30f2>
lea    -0x1(%r13),%eax
test   %eax,%r13d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2e64>
cmp    $0x10,%r9d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ef6>
lea    0x4(%r9),%eax
and    $0x1c,%eax
cmp    $0x2,%r9
mov    0x20(%rsp),%rcx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f0f>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3136>
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
mov    0x50(%rsp),%r9
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
add    $0x108,%rax
lea    0x108(%rsp),%rcx
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
lea    -0x1fb739(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6327>
mov    %dl,0x48(%r12,%rdi,1)
incq   0x50(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ba0>
mov    0x98(%rsp),%rax
movq   $0x0,(%rax)
mov    0x10(%rsp),%rcx
mov    %rcx,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52ec>
mov    $0x18,%eax
cmp    $0x19,%r8
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2c3f>
mov    $0x20,%eax
cmp    $0x21,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2a7d>
lea    (%r11,%rax,8),%r12
lea    -0x1(%r14),%eax
cmp    $0x5,%eax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d41>
lea    -0x206787(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x18dd>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
movzbl (%r12,%rbp,1),%eax
mov    %r8,%rsi
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d72>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x368f>
and    0x8(%rcx),%rbp
mov    (%rcx),%rax
cmp    %rbx,%rbp
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2df7>
mov    0x50(%rsp),%rdx
mov    %rdx,(%rcx)
mov    %rcx,%rdx
mov    0x98(%rsp),%rcx
movq   $0x1,(%rcx)
mov    %rax,0x8(%rcx)
mov    %rdx,0x10(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52ec>
lea    (%r8,%r11,8),%rax
mov    (%r8,%r11,8),%rcx
mov    0x50(%rsp),%rdx
mov    %rdx,(%r8,%r11,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d88>
lea    0x0(,%rbp,4),%rax
add    %rbp,%rax
mov    (%r12,%rax,1),%ecx
movzbl 0x4(%r12,%rax,1),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d2d>
lea    0x0(,%rbp,2),%rax
add    %rbp,%rax
movzwl (%r12,%rax,1),%ecx
movzbl 0x2(%r12,%rax,1),%eax
shl    $0x10,%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d31>
mov    (%r12,%rbp,4),%eax
mov    %r8,%rsi
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d72>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x368f>
movzwl (%r12,%rbp,2),%eax
mov    %r8,%rsi
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d72>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x368f>
lea    0x0(,%rbp,2),%rax
add    %rbp,%rax
mov    (%r12,%rax,2),%ecx
movzwl 0x4(%r12,%rax,2),%eax
shl    $0x20,%rax
or     %rcx,%rax
mov    %r8,%rsi
cmp    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d72>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x368f>
lea    (%r12,%rbp,8),%rax
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x368f>
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2da4>
mov    %rbp,%rsi
lea    (%r11,%rsi,8),%rax
mov    (%r11,%rsi,8),%rcx
mov    0x50(%rsp),%rdx
mov    %rdx,(%r11,%rsi,8)
mov    0x98(%rsp),%rdx
movq   $0x1,(%rdx)
mov    %rcx,0x8(%rdx)
mov    %rax,0x10(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52ec>
lea    -0x1(%r14),%eax
cmp    $0x5,%eax
mov    %r10,0x48(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x362b>
lea    -0x2068d5(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x18f5>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    %r12,0x38(%rsp)
mov    %r12,%rdi
mov    %r8,%rsi
mov    %rdx,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<1>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x35e6>
mov    0x38(%rsp),%rax
movzbl (%rax,%rsi,1),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x35cb>
setbe  %cl
mov    %rax,%r12
mov    0x50(%rsp),%rdx
cmovbe %rdx,%r12
cmovbe %rax,%rdx
mov    %rbp,%r15
cmova  %rbx,%r15
cmovb  %rbx,%rbp
movzbl %cl,%ebx
cmp    $0x40,%r13b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f1b>
mov    %rdx,0x50(%rsp)
mov    0x20(%rsp),%rdx
lock addq $0x10,0x200(%rdx)
lock incq 0x208(%rdx)
lock incq 0x218(%rdx)
mov    0x80(%rdx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f53>
mov    (%rax),%rcx
mov    %rcx,0x80(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x400d>
mov    $0x2,%eax
mov    %r9,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30b7>
and    $0xfffffffc,%ecx
cmpb   $0xf,0x28(%rsp)
mov    $0x18,%eax
cmovne %rcx,%rax
lea    0x8(%r14),%r13
imul   %rax,%r13
lea    0xc(%r13),%rdx
mov    %edx,%r12d
and    $0xfffffff0,%r12d
mov    0x20(%rsp),%rcx
lock add %r12,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
lea    -0x1f9910(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl 0x0(%r13,%rax,1),%ebp
cmp    $0xff,%rbp
mov    %r8,0x18(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e95>
cmp    $0x3d,%bpl
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6619>
mov    0x10(%rcx,%rbp,8),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d32>
mov    (%r15),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e80>
cmp    $0x18,%r9d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3095>
mov    $0x18,%eax
cmp    $0x10,%r9d
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30a6>
lea    0x3(%r9),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30b7>
mov    %r15,0xa8(%rsp)
mov    %rdx,0xb0(%rsp)
mov    %rbp,0xb8(%rsp)
mov    %r12,0xc0(%rsp)
movzbl %r14b,%edx
lea    0xa8(%rsp),%rcx
mov    $0x2,%r8d
mov    0x20(%rsp),%rdi
mov    0x10(%rsp),%rsi
call   ADDR <expanse_trie::mutate_map::build_map_leaf::<false>>
mov    0x78(%rsp),%rax
test   %rax,%rax
mov    0x1c0(%rsp),%r10
mov    0x40(%rsp),%r11
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ff5>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x108,%rax
lea    0x108(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f99>
xchg   %ax,%ax
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2ff5>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2fcb>
mov    0x8(%rsi),%rdi
lea    -0x1fbb29(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f90>
mov    %rsi,(%r10,%rdi,8)
mov    0x50(%r10),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6327>
mov    %dl,0x48(%r10,%rdi,1)
incq   0x50(%r10)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2f90>
shl    $0x3,%ebx
mov    0x10(%rsp),%rax
add    (%rax),%rbx
mov    0x98(%rsp),%rax
movq   $0x0,(%rax)
mov    %rbx,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52ec>
lea    0x0(,%rdx,8),%esi
lea    0x20(%rsi),%r10
mov    %r10d,%edx
and    $0xffffffe0,%edx
mov    0x20(%rsp),%rcx
lock add %rdx,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
lea    -0x1f9aa4(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rdx,%rax,1),%ebp
cmp    $0xff,%rbp
mov    %r8,0x18(%rsp)
mov    %r9,0x70(%rsp)
mov    %rsi,0xf8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x46f4>
cmp    $0x3d,%bpl
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6619>
mov    0x10(%rcx,%rbp,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x554e>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x56c7>
xor    %eax,%eax
cmp    $0xffffffff,%r13d
sete   %al
lea    0x20(,%rax,4),%rax
xor    %ecx,%ecx
cmp    $0x19,%r9d
setae  %cl
lea    0x18(,%rcx,8),%rcx
cmp    %ecx,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x30f2>
mov    0x20(%rsi,%r14,8),%r15
lea    (%r15,%r8,8),%rsi
lea    (%r15,%r8,8),%rdi
add    $0x8,%rdi
sub    %r8,%r9
shl    $0x3,%r9
mov    %r9,%rdx
mov    %r8,%r13
call   *0x16f461(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x58(%rsp),%rsi
mov    0x50(%rsp),%rax
mov    %rax,(%r15,%r13,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33f7>
lea    -0x1(%r13),%eax
test   %eax,%r13d
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x310f>
cmp    $0xf,%r9d
mov    0x20(%rsp),%rcx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x311a>
lea    0x4(%r9),%eax
and    $0x1c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3136>
lea    0x1(%r9),%rax
mov    0x20(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3136>
mov    $0x18,%eax
cmp    $0x18,%r9d
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3136>
xor    %eax,%eax
cmp    $0xffffffff,%r13d
sete   %al
lea    0x20(,%rax,4),%rax
lea    0x0(,%rax,8),%r10d
lea    0x8(%r10),%r12
and    $0xfffffffffffffff0,%r12
lock add %r12,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
lea    -0x1f9bbc(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%r10,%rdx,1),%ebp
cmp    $0xff,%rbp
mov    %r8,0x18(%rsp)
mov    %r14,0x38(%rsp)
mov    %r9,0x30(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x31a3>
cmp    $0x3d,%bpl
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6619>
mov    0x10(%rcx,%rbp,8),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x320a>
mov    (%r15),%rax
mov    %rax,0x10(%rcx,%rbp,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3298>
cmp    $0x1,%eax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x32b5>
movq   $0x0,0xa8(%rsp)
lea    0xa8(%rsp),%rdi
mov    $0x10,%esi
mov    %r10,%r14
mov    %r10,%rdx
call   *0x16f37f(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
mov    0x1c0(%rsp),%r12
mov    0x40(%rsp),%rbp
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6692>
mov    0xa8(%rsp),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6692>
mov    %r15,%rdi
xor    %esi,%esi
mov    %r14,%rdx
call   *0x16f35b(%rip)        # ADDR <memset@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x32df>
cmp    $0x21,%eax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x32b5>
mov    %r10,%r14
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x645b>
mov    %rax,%r15
mov    0x20(%rsp),%rcx
mov    0x220(%rcx),%rax
mov    %rax,(%r15)
mov    %rbp,0x8(%r15)
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
mov    0x20(%rsp),%rsi
mov    0x10(%rsi,%rbp,8),%rsi
mov    %rsi,0x40(%r15,%rcx,1)
mov    0x20(%rsp),%rcx
mov    %rdx,0x10(%rcx,%rbp,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3260>
add    $0x40,%r15
mov    %r14,%r10
mov    %r15,%rdi
xor    %esi,%esi
mov    %r10,%rdx
call   *0x16f2ba(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x1c0(%rsp),%r12
mov    0x40(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x32df>
mov    $0x1,%esi
mov    %r10,%r14
mov    %r10,%rdi
call   *0x16f292(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r15
test   %rax,%rax
mov    0x1c0(%rsp),%r12
mov    0x40(%rsp),%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6692>
test   %r13d,%r13d
mov    0x18(%rsp),%r13
mov    0x58(%rsp),%rsi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33e4>
mov    0x38(%rsp),%rax
mov    0x20(%rsi,%rax,8),%r14
mov    %r14,0x60(%rsp)
lea    0x0(,%r13,8),%ebp
mov    %r15,%rdi
mov    %r14,%rsi
mov    %rbp,%rdx
call   *0x16f158(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    (%r15,%rbp,1),%rdi
add    $0x8,%rdi
add    %r14,%rbp
mov    0x30(%rsp),%r14
mov    %r14,%rdx
sub    %r13,%rdx
shl    $0x3,%rdx
mov    %rbp,%rsi
call   *0x16f135(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x60(%rsp),%rsi
test   %rsi,%rsi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6402>
cmp    $0x3,%r14d
mov    0x20(%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x335b>
mov    0x40(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3381>
cmp    $0x11,%r14d
mov    0x40(%rsp),%rbp
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3370>
add    $0x3,%r14d
and    $0x3c,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3381>
xor    %eax,%eax
cmp    $0x19,%r14d
setae  %al
lea    0x18(,%rax,8),%r14
lea    0x8(,%r14,8),%eax
and    $0xfffffff0,%eax
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
lea    0x0(,%r14,8),%eax
lea    -0x1f9e02(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rax,%rdx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33d1>
cmp    $0x3d,%dil
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x662e>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rsi)
mov    %rsi,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x33da>
mov    %rsi,%rdi
call   *0x16f0ae(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x18(%rsp),%r13
mov    0x58(%rsp),%rsi
mov    0x50(%rsp),%rax
mov    %rax,(%r15,%r13,8)
mov    0x38(%rsp),%r14
mov    %r15,0x20(%rsi,%r14,8)
mov    $0x1,%eax
mov    %ebx,%ecx
shl    %cl,%rax
mov    %ebx,%ecx
shr    $0x3,%ecx
and    $0x18,%ecx
or     %rax,(%rsi,%rcx,1)
mov    0x10(%rsp),%rdx
mov    0x8(%rdx),%rax
lea    0x1(%rax),%ecx
mov    %cl,0x8(%rdx)
cmpb   $0x1,0x28(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3461>
mov    %rdx,%rcx
shr    $0x8,%rbx
mov    %rbx,0x40(%r12)
mov    (%rcx),%rcx
mov    %rcx,0x58(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x60(%r12)
movzbl %al,%eax
add    $0x2,%eax
mov    %ax,0x70(%r12)
mov    %rdx,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
shl    $0x3,%r13d
add    0x20(%rsi,%r14,8),%r13
mov    0x78(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x34f8>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x108,%rax
lea    0x108(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3499>
nop
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x34f8>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x34cb>
mov    0x8(%rsi),%rdi
lea    -0x1fc029(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3490>
mov    %rsi,(%r12,%rdi,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6327>
mov    %dl,0x48(%r12,%rdi,1)
incq   0x50(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3490>
mov    0x98(%rsp),%rax
movq   $0x0,(%rax)
mov    %r13,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52ec>
mov    %r12,%rdi
mov    %r8,%rsi
mov    %rdx,0x30(%rsp)
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<5>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3680>
lea    (%rsi,%rsi,4),%rax
mov    (%r12,%rax,1),%ecx
movzbl 0x4(%r12,%rax,1),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3622>
mov    %r12,%rdi
mov    %r8,%rsi
mov    %rdx,0x30(%rsp)
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<3>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3680>
lea    (%rsi,%rsi,2),%rax
movzwl (%r12,%rax,1),%ecx
movzbl 0x2(%r12,%rax,1),%eax
shl    $0x10,%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3626>
mov    %r12,0x38(%rsp)
mov    %r12,%rdi
mov    %r8,%rsi
mov    %rdx,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<4>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x35e6>
mov    0x38(%rsp),%rax
mov    (%rax,%rsi,4),%eax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x35cb>
mov    %r12,0x38(%rsp)
mov    %r12,%rdi
mov    %r8,%rsi
mov    %rdx,%r12
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<2>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x35e6>
mov    0x38(%rsp),%rax
movzwl (%rax,%rsi,2),%eax
cmp    %rax,%r12
mov    0x18(%rsp),%r11
mov    0x48(%rsp),%r10
mov    %r12,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d77>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x368f>
mov    0x18(%rsp),%r11
mov    0x48(%rsp),%r10
mov    %r12,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x368f>
mov    %r12,%rdi
mov    %r8,%rsi
mov    %rdx,0x30(%rsp)
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<6>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3680>
lea    (%rsi,%rsi,2),%rax
mov    (%r12,%rax,2),%ecx
movzwl 0x4(%r12,%rax,2),%eax
shl    $0x20,%rax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3666>
mov    %r12,%rdi
mov    %r8,%rsi
mov    %rdx,0x30(%rsp)
call   ADDR <expanse_trie::leaf::lower_bound_fixed::<7>>
lea    0x1(%rbp),%r8
mov    %rax,%rsi
cmp    %r8,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3680>
lea    (%r12,%rsi,8),%rax
sub    %rsi,%rax
mov    (%rax),%ecx
movzwl 0x4(%rax),%edx
movzbl 0x6(%rax),%eax
shl    $0x20,%rdx
or     %rcx,%rdx
shl    $0x30,%rax
or     %rdx,%rax
mov    0x30(%rsp),%rdx
cmp    %rdx,%rax
mov    0x18(%rsp),%r11
mov    0x48(%rsp),%r10
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x2d77>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x368f>
mov    0x18(%rsp),%r11
mov    0x48(%rsp),%r10
mov    0x30(%rsp),%rdx
mov    %rsi,0x38(%rsp)
cmp    $0x1,%r14
sete   %r9b
cmp    $0x1,%r14d
mov    $0x19,%eax
mov    $0x20,%ecx
cmovne %rcx,%rax
cmp    %rax,%r8
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3723>
lea    0x2(%rbp),%rdi
test   %rbp,%rbp
mov    %rdi,0xd8(%rsp)
jle    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x398e>
cmp    $0xf,%rbp
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x39c3>
lea    0x5(%rbp),%eax
and    $0x1c,%eax
cmp    $0x2,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3b12>
lea    0x8(%r14),%r13
lea    0x0(,%r13,2),%edi
mov    %rdx,%rsi
mov    0x58(%rsp),%rdx
mov    (%rdx),%ecx
mov    0x3(%rdx),%edx
mov    %edx,0x93(%rsp)
mov    %rsi,%rdx
mov    %rdi,%rsi
mov    %ecx,0x90(%rsp)
mov    $0x2,%ecx
mov    %rcx,0x60(%rsp)
mov    0x20(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4087>
mov    %rax,0x48(%rsp)
mov    %rdx,0x30(%rsp)
mov    0x58(%rsp),%r12
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %ecx,0x93(%rsp)
mov    %eax,0x90(%rsp)
lea    0xe0(%rsp),%rdi
mov    0x10(%rsp),%rsi
mov    %r14d,%edx
mov    %r8,%rcx
mov    %r8,0x70(%rsp)
call   *0x16f081(%rip)        # ADDR <_DYNAMIC+0x590>
mov    0x10(%rsp),%rax
mov    (%rax),%rax
mov    %rax,0x60(%rsp)
and    $0xfffffffffffffffc,%rbp
add    $0x4,%rbp
cmpq   $0x21,0x70(%rsp)
mov    $0x20,%eax
cmovb  %rax,%rbp
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %ecx,0x103(%rsp)
mov    %eax,0x100(%rsp)
mov    0xf0(%rsp),%rsi
cmp    0x38(%rsp),%rsi
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63b8>
mov    %rsi,0x18(%rsp)
cmp    0xe0(%rsp),%rsi
mov    0x1c0(%rsp),%r12
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x37da>
lea    0xe0(%rsp),%rdi
call   *0x16f5be(%rip)        # ADDR <_DYNAMIC+0xb40>
mov    0x38(%rsp),%rax
mov    %rax,%rcx
shl    $0x4,%rcx
add    0xe8(%rsp),%rcx
mov    0x18(%rsp),%rdx
sub    %rax,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3815>
lea    0x10(%rcx),%rdi
shl    $0x4,%rdx
mov    %rcx,0x38(%rsp)
mov    0x38(%rsp),%rsi
call   *0x16ed30(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x38(%rsp),%rcx
mov    0x18(%rsp),%rax
lea    0x1(%rax),%r8
mov    0x30(%rsp),%rax
mov    %rax,(%rcx)
mov    0x50(%rsp),%rax
mov    %rax,0x8(%rcx)
mov    %r8,0xf0(%rsp)
mov    0x28(%rsp),%rax
cmp    %r14b,%al
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3910>
mov    %r14,%rsi
xor    $0x7,%rsi
sub    %r14b,%al
movzbl %al,%edi
dec    %rdi
lea    -0x4(%r13),%eax
movzbl %al,%eax
mov    0x10(%rsp),%rcx
add    %rax,%rcx
add    $0x7,%rcx
xor    %eax,%eax
lea    0x1640ff(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    %rsi,%rdi
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65f9>
mov    %rax,%r9
shl    $0x8,%r9
movzbl 0x1(%rcx,%rdi,1),%eax
or     %r9,%rax
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3880>
mov    %r15d,%ecx
shl    %cl,%rax
mov    0xe8(%rsp),%rcx
mov    %r8,%rsi
mov    %rcx,%rdx
and    $0x7,%rsi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x38cc>
mov    %rcx,%rdx
nopl   0x0(%rax,%rax,1)
or     %rax,(%rdx)
add    $0x10,%rdx
dec    %rsi
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x38c0>
cmpq   $0x7,0x18(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3908>
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
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x38e0>
mov    0xf0(%rsp),%r8
mov    0xe8(%rsp),%rcx
cmp    0x48(%rsp),%r8
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3940>
cmp    $0x1,%r14d
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x39dc>
mov    0x20(%rsp),%rdi
mov    0x10(%rsp),%rsi
mov    %rcx,%rdx
mov    %r8,%rcx
call   ADDR <expanse_trie::mutate_map::build_bitmap_leaf_map::<false>>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3952>
mov    0x20(%rsp),%rdi
mov    0x10(%rsp),%rsi
mov    %r14d,%edx
call   ADDR <expanse_trie::mutate_map::build_map_leaf::<false>>
cmp    %r14b,0x28(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5136>
mov    0x28(%rsp),%rax
movzbl %al,%edx
cmp    $0x8,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x646b>
sub    %r14,%rdx
lea    (%rsp,%r14,1),%rsi
add    $0x100,%rsi
mov    0x58(%rsp),%rdi
add    %r14,%rdi
call   *0x16eae7(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5136>
lea    0x8(%r14),%r13
mov    %r8,%rsi
imul   %r13,%rsi
mov    0x58(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x93(%rsp)
mov    %eax,0x90(%rsp)
mov    %r8,0x60(%rsp)
mov    %rdi,%rax
mov    0x20(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4087>
mov    %rdx,%rsi
xor    %eax,%eax
cmp    $0x17,%rbp
setae  %al
lea    0x18(,%rax,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3b15>
shl    $0x4,%r8
mov    -0x10(%rcx,%r8,1),%rdx
xor    (%rcx),%rdx
mov    0x28(%rsp),%rax
cmp    $0x7,%al
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3a10>
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
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3c49>
add    %rcx,%r8
lea    0xa8(%rsp),%rdi
mov    %rcx,%rsi
mov    %r8,%rdx
call   ADDR <<alloc::vec::Vec<(u64, u64)> as alloc::vec::spec_from_iter::SpecFromIter<(u64, u64), core::iter::adapters::map::Map<core::slice::iter::Iter<(u64, u64)>, expanse_trie::mutate_map::map_insert_with_path_occ<false, true, false>::{closure#0}>>>::from_iter>
cmpq   $0x0,0xf0(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66a0>
mov    0xe8(%rsp),%rax
mov    (%rax),%rax
mov    %rax,0x58(%rsp)
mov    0xb0(%rsp),%rdx
mov    0xb8(%rsp),%rcx
mov    0x20(%rsp),%rdi
mov    0x10(%rsp),%r13
mov    %r13,%rsi
call   ADDR <expanse_trie::mutate_map::build_bitmap_leaf_map::<false>>
mov    0x28(%rsp),%r15
movzbl %r15b,%edx
mov    %r13,%rdi
mov    $0x1,%esi
mov    0x58(%rsp),%rcx
call   *0x16ed52(%rip)        # ADDR <_DYNAMIC+0x598>
cmp    $0x1,%r15b
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3af0>
mov    %rbx,%rax
shr    $0x8,%rax
mov    %rax,0x40(%r12)
mov    0x10(%rsp),%rcx
mov    (%rcx),%rax
mov    %rax,0x58(%r12)
movq   $0x0,0x60(%r12)
movzwl 0xf0(%rsp),%eax
mov    %ax,0x70(%r12)
mov    %rcx,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
cmpq   $0x0,0xa8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5136>
mov    0xb0(%rsp),%rdi
call   *0x16e97b(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5136>
mov    %rdx,%rsi
mov    0x38(%rsp),%r15
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
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3bf7>
add    $0x4,%ebp
and    $0x3c,%ebp
cmp    $0x11,%r8
cmovb  %ebp,%ecx
lea    (%r11,%rcx,8),%r12
lea    -0x1(%r14),%eax
cmp    $0x5,%eax
mov    %rsi,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5a98>
lea    -0x207679(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x190d>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
jmp    *%rax
mov    %r8,%r13
sub    %r15,%r13
mov    0x40(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3be0>
lea    (%r11,%r15,8),%rsi
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %rdx,0x30(%rsp)
lea    0x0(,%r13,8),%rdx
mov    %r10,0x48(%rsp)
mov    %r8,0x70(%rsp)
call   *0x16e988(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    (%r12,%r15,1),%rsi
lea    (%r12,%r15,1),%rdi
inc    %rdi
mov    %r13,%rdx
call   *0x16e974(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x70(%rsp),%r8
mov    0x30(%rsp),%rdx
mov    0x48(%rsp),%r10
mov    0x18(%rsp),%r11
mov    0x50(%rsp),%rax
mov    %rax,(%r11,%r15,8)
mov    %dl,(%r12,%r15,1)
mov    0x10(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6008>
cmp    $0x11,%r8
mov    %rsi,%rdx
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3ca2>
lea    0x4(%rbp),%esi
and    $0x3c,%esi
lea    0x8(%r14),%r13
imul   %r13,%rsi
mov    0x58(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x93(%rsp)
mov    %eax,0x90(%rsp)
cmp    $0xe,%rbp
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x406c>
lea    0x5(%rbp),%eax
and    $0x1c,%eax
mov    %r8,0x60(%rsp)
mov    0x20(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4087>
mov    $0x3f,%cl
sub    %al,%cl
shr    $0x3,%cl
inc    %cl
mov    0x28(%rsp),%r12
cmp    $0x8,%r12b
movzbl %cl,%r15d
movzbl %r12b,%eax
mov    %eax,0x50(%rsp)
cmovae %eax,%r15d
mov    0x20(%rsp),%rdx
lock addq $0x40,0x200(%rdx)
lock incq 0x208(%rdx)
lock incq 0x218(%rdx)
mov    0x18(%rdx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f9c>
mov    (%rax),%rcx
mov    %rcx,0x18(%rdx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x500b>
lea    0x8(%r14),%r13
mov    0x58(%rsp),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x93(%rsp)
mov    %eax,0x90(%rsp)
cmp    $0x18,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4066>
lea    0x0(,%r13,8),%eax
lea    (%rax,%rax,2),%rsi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x406c>
mov    0x50(%rsp),%rax
mov    %rax,(%rsi)
mov    0xa0(%rsp),%r12
mov    (%r12),%eax
mov    0x3(%r12),%ecx
mov    %ecx,0x83(%rsp)
mov    %eax,0x80(%rsp)
mov    0x40(%rsp),%r10
mov    0x38(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ccd>
lea    -0x207884(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x18a5>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %bl,0x80(%rsp,%r11,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ccd>
cmp    $0x101,%r13d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e95>
mov    %rdx,0x48(%rsp)
mov    %r13,0x60(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x645b>
mov    %rax,%r15
mov    0x20(%rsp),%r13
mov    0x220(%r13),%rax
mov    %rax,(%r15)
mov    %rbp,0x8(%r15)
mov    %r15,0x220(%r13)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3dc0>
mov    %rcx,%rax
dec    %rax
mov    %rax,%rsi
imul   %r12,%rsi
lea    (%r15,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%r13,%rbp,8),%r8
mov    %r8,0x40(%r15,%rsi,1)
mov    %rdi,0x10(%r13,%rbp,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3d9a>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e77>
lea    -0x4(%rax),%rcx
mov    0x48(%rsp),%r9
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
mov    0x10(%r13,%rbp,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x10(%r13,%rbp,8)
lea    (%r9,%rdi,1),%r10
mov    0x10(%r13,%rbp,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%r13,%rbp,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%r13,%rbp,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%r13,%rbp,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%r13,%rbp,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%r13,%rbp,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3e22>
add    $0x40,%r15
mov    0x60(%rsp),%r13
mov    %r15,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0x16e6d2(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x40(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3eb3>
mov    $0x10,%esi
mov    %r13,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%r15
test   %rax,%rax
mov    0x40(%rsp),%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6513>
mov    0x28(%rsp),%rdx
lea    0x1(%rdx),%r8
lea    0x20(,%r8,8),%eax
and    $0xffffffe0,%eax
cmp    $0xf,%dl
mov    $0xc0,%r13d
cmovne %rax,%r13
add    %r15,%r13
mov    0x58(%rsp),%r10
test   %r10,%r10
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f20>
lea    0x0(,%r10,8),%rdx
mov    %r15,%rdi
mov    0x18(%rsp),%rsi
mov    0x16e578(%rip),%r12        # ADDR <memcpy@GLIBC_2.14>
call   *%r12
mov    0x58(%rsp),%rdx
imul   %r14,%rdx
mov    %r13,%rdi
mov    0xa0(%rsp),%rsi
call   *%r12
mov    0x28(%rsp),%rdx
mov    0x58(%rsp),%r10
lea    0x1(%rdx),%r8
mov    0x50(%rsp),%rax
mov    %rax,(%r15,%r10,8)
mov    0x38(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x532d>
lea    -0x207ae0(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x186d>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %bl,0x0(%r13,%r10,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5373>
mov    %rdi,0xa0(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x645b>
mov    0x20(%rsp),%r8
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
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4001>
lea    -0x30(%rcx),%rsi
mov    0x80(%r8),%rdi
mov    %rdi,-0x30(%rcx)
mov    %rsi,0x80(%r8)
add    $0xffffffffffffffc0,%rcx
add    $0x4,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x3f9f>
add    $0x40,%rax
mov    0xa0(%rsp),%rdi
mov    0x50(%rsp),%rcx
mov    %rcx,(%rax)
mov    %r12,0x8(%rax)
movw   $0x0,0x6c(%rsp)
movl   $0x0,0x68(%rsp)
movzbl %r14b,%ecx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b51>
lea    -0x207b8a(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0x18c1>
movslq (%rdx,%rcx,4),%rcx
add    %rdx,%rcx
mov    0x1c0(%rsp),%r11
mov    0x78(%rsp),%rdx
jmp    *%rcx
mov    %r15b,0x68(%rsp)
mov    %bpl,0x69(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b9c>
mov    %r13d,%esi
shl    $0x5,%esi
mov    0x20(%rsp),%rcx
xor    %eax,%eax
cmp    $0x17,%rbp
setae  %al
lea    0x18(,%rax,8),%rax
mov    %r8,0x60(%rsp)
imul   %rax,%r13
lea    0xf(%r13),%r12
and    $0xfffffffffffffff0,%r12
lock add %r12,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
cmp    $0x177,%r13
mov    %r10,0x48(%rsp)
mov    %rdx,0x30(%rsp)
mov    %r9b,0xf8(%rsp)
mov    %rsi,0x70(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4215>
lea    -0x1fab2d(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl 0x0(%r13,%rax,1),%r15d
cmp    $0xff,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4106>
cmp    $0x3d,%r15b
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6657>
mov    0x10(%rcx,%r15,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x416e>
mov    (%rdi),%rax
mov    %rax,0x10(%rcx,%r15,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x41f5>
cmp    $0xf,%r13
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4215>
movq   $0x0,0xa8(%rsp)
lea    0xa8(%rsp),%rdi
mov    $0x10,%esi
mov    %r13,%rdx
call   *0x16e41e(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
mov    0x1c0(%rsp),%r12
mov    0x38(%rsp),%r15
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6513>
mov    0xa8(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6513>
xor    %esi,%esi
mov    %r13,%rdx
mov    %rdi,%r13
call   *0x16e3fa(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r13,%r10
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x423c>
cmp    $0x101,%r13
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4215>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x645b>
mov    %rax,%rdi
mov    0x20(%rsp),%rcx
mov    0x220(%rcx),%rax
mov    %rax,(%rdi)
mov    %r15,0x8(%rdi)
mov    %rdi,0x220(%rcx)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%eax
nopl   (%rax)
dec    %rax
mov    %rax,%rcx
imul   %r12,%rcx
lea    (%rdi,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x20(%rsp),%rsi
mov    0x10(%rsi,%r15,8),%rsi
mov    %rsi,0x40(%rdi,%rcx,1)
mov    0x20(%rsp),%rcx
mov    %rdx,0x10(%rcx,%r15,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x41c0>
add    $0x40,%rdi
xor    %esi,%esi
mov    %r13,%rdx
mov    %rdi,%r15
call   *0x16e35d(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r15,%r10
mov    0x1c0(%rsp),%r12
mov    0x38(%rsp),%r15
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x423c>
mov    $0x1,%esi
mov    %r13,%rdi
call   *0x16e335(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r10
test   %rax,%rax
mov    0x1c0(%rsp),%r12
mov    0x38(%rsp),%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6513>
lea    -0x1(%r14),%eax
cmp    $0x5,%eax
mov    %r10,%r13
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4527>
lea    -0x207d3b(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0x1925>
movslq (%rcx,%rax,4),%rax
add    %rcx,%rax
mov    0x50(%rsp),%r14
mov    0x18(%rsp),%r8
jmp    *%rax
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4287>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16e1f1(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    0x60(%rsp),%r14
mov    %r14,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x42b6>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16e1c2(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4633>
mov    %r14,%rax
lea    (%r8,%r14,8),%r14
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x465c>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x493a>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x42ff>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16e179(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    0x60(%rsp),%r14
mov    %r14,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x432e>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16e14a(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x45a6>
lea    (%r8,%r14,8),%rsi
cmp    $0x2,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x45ca>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4748>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4374>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16e104(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    0x60(%rsp),%r14
mov    %r14,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x43a3>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16e0d5(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x45d5>
lea    (%r8,%r14,8),%rsi
cmp    $0x2,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x45f9>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47f4>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x43e9>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16e08f(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    0x60(%rsp),%r14
mov    %r14,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4418>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16e060(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4604>
lea    (%r8,%r14,8),%rsi
cmp    $0x2,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4628>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x489b>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x445e>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16e01a(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    0x60(%rsp),%r14
mov    %r14,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x448d>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16dfeb(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4667>
lea    (%r8,%r14,8),%rsi
cmp    $0x2,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x468b>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49c5>
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x44d3>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16dfa5(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    0x60(%rsp),%r14
mov    %r14,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4502>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16df76(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4696>
lea    (%r8,%r14,8),%rsi
cmp    $0x2,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x46ba>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a62>
test   %r15,%r15
mov    0x50(%rsp),%r14
mov    0x18(%rsp),%r8
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4552>
lea    0x0(,%r15,8),%rdx
mov    %r10,%rdi
mov    %r8,%rsi
call   *0x16df26(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
mov    %r14,(%r10,%r15,8)
mov    0x60(%rsp),%r14
mov    %r14,%rdx
sub    %r15,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4581>
lea    (%r10,%r15,8),%rdi
lea    (%r8,%r15,8),%rsi
add    $0x8,%rdi
shl    $0x3,%rdx
call   *0x16def7(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
lea    0x1(%rbp),%rax
cmp    $0x3,%rax
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x46c5>
lea    (%r8,%r14,8),%rsi
cmp    $0x2,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x46e9>
add    $0x2,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b13>
cmp    $0x11,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4720>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4737>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4748>
cmp    $0x11,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47cc>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47e3>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x47f4>
cmp    $0x11,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4873>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x488a>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x489b>
cmp    $0x11,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4910>
mov    %r14,%rax
lea    0x20(,%rbp,8),%r14d
and    $0x1e0,%r14d
add    %r8,%r14
cmp    $0x10,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4927>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x493a>
cmp    $0x11,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x499d>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49b4>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49c5>
cmp    $0x11,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a3a>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a51>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a62>
cmp    $0x11,%r14
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4aeb>
lea    0x20(,%rbp,8),%esi
and    $0x1e0,%esi
add    %r8,%rsi
cmp    $0x10,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b02>
add    $0x5,%ebp
and    $0x1c,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b13>
mov    $0x10,%esi
mov    %rdx,%rdi
mov    %rdx,%rbp
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,0x48(%rsp)
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x56d4>
mov    $0x10,%edi
mov    %rbp,%rsi
call   *0x16ddb8(%rip)        # ADDR <_DYNAMIC+0x280>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r14
setae  %al
lea    0x18(,%rax,8),%rbp
mov    %r10,0x50(%rsp)
lea    (%r10,%rbp,8),%r15
mov    0x38(%rsp),%r13
test   %r13,%r13
mov    %rsi,0xc8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4781>
lea    0x0(,%r13,4),%r14
add    %r13,%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16dcf6(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4784>
xor    %r14d,%r14d
mov    0x40(%rsp),%rbp
mov    0x30(%rsp),%rdx
mov    %rdx,%rcx
shr    $0x20,%rcx
mov    0x60(%rsp),%rax
mov    %edx,(%r15,%r14,1)
mov    %cl,0x4(%r15,%r14,1)
sub    %r13,%rax
mov    0x50(%rsp),%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bb6>
add    %r14,%r15
mov    0xc8(%rsp),%rsi
add    %r14,%rsi
add    $0x5,%r15
lea    (%rax,%rax,4),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ba2>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r14
setae  %al
lea    0x18(,%rax,8),%rbp
mov    %r10,0x50(%rsp)
lea    (%r10,%rbp,8),%r15
mov    0x38(%rsp),%r13
test   %r13,%r13
mov    %rsi,0xc8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x482d>
lea    0x0(,%r13,2),%r14
add    %r13,%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16dc4a(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4830>
xor    %r14d,%r14d
mov    0x40(%rsp),%rbp
mov    0x30(%rsp),%rdx
mov    %edx,%ecx
shr    $0x10,%ecx
mov    0x60(%rsp),%rax
mov    %dx,(%r15,%r14,1)
mov    %cl,0x2(%r15,%r14,1)
sub    %r13,%rax
mov    0x50(%rsp),%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bb6>
add    %r14,%r15
mov    0xc8(%rsp),%rsi
add    %r14,%rsi
add    $0x3,%r15
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ae2>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r14
setae  %al
lea    0x18(,%rax,8),%rbp
mov    %r10,0x50(%rsp)
lea    (%r10,%rbp,8),%r15
mov    0x38(%rsp),%r13
test   %r13,%r13
mov    %rsi,0xc8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x48d1>
lea    0x0(,%r13,4),%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16dba6(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x48d4>
xor    %r14d,%r14d
mov    0x40(%rsp),%rbp
mov    0x60(%rsp),%rdx
mov    0x30(%rsp),%rax
mov    %eax,(%r15,%r14,1)
sub    %r13,%rdx
mov    0x50(%rsp),%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bb6>
add    %r14,%r15
mov    0xc8(%rsp),%rsi
add    %r14,%rsi
add    $0x4,%r15
shl    $0x2,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ba2>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%r14
add    $0xc0,%r14
xor    %eax,%eax
cmpq   $0x18,0x60(%rsp)
setae  %al
lea    0x18(,%rax,8),%rbp
lea    (%r10,%rbp,8),%r15
mov    0x38(%rsp),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x495f>
mov    %r15,%rdi
mov    %r14,%rsi
mov    %rbp,%rdx
call   *0x16db19(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
mov    0x18(%rsp),%r8
mov    0x60(%rsp),%rdx
mov    0x30(%rsp),%rax
mov    %al,(%r15,%rbp,1)
sub    %rbp,%rdx
mov    0x40(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bb6>
mov    0x38(%rsp),%rax
add    %rax,%r15
add    %rax,%r14
inc    %r15
mov    %r15,%rdi
mov    %r14,%rsi
call   *0x16dadb(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r13,%r10
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bb1>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r14
setae  %al
lea    0x18(,%rax,8),%rbp
mov    %r10,0x50(%rsp)
lea    (%r10,%rbp,8),%r15
mov    0x38(%rsp),%r13
test   %r13,%r13
mov    %rsi,0xc8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49fb>
lea    0x0(,%r13,2),%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16da7c(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x49fe>
xor    %r14d,%r14d
mov    0x40(%rsp),%rbp
mov    0x60(%rsp),%rdx
mov    0x30(%rsp),%rax
mov    %ax,(%r15,%r14,1)
sub    %r13,%rdx
mov    0x50(%rsp),%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bb6>
add    %r14,%r15
mov    0xc8(%rsp),%rsi
add    %r14,%rsi
add    $0x2,%r15
add    %rdx,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ba2>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r14
setae  %al
lea    0x18(,%rax,8),%rbp
mov    %r10,0x50(%rsp)
lea    (%r10,%rbp,8),%r15
mov    0x38(%rsp),%r13
test   %r13,%r13
mov    %rsi,0xc8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a9c>
lea    0x0(,%r13,2),%rax
lea    (%rax,%rax,2),%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16d9db(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4a9f>
xor    %r14d,%r14d
mov    0x40(%rsp),%rbp
mov    0x30(%rsp),%rdx
mov    %rdx,%rcx
shr    $0x20,%rcx
mov    0x60(%rsp),%rax
mov    %edx,(%r15,%r14,1)
mov    %cx,0x4(%r15,%r14,1)
sub    %r13,%rax
mov    0x50(%rsp),%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bb6>
add    %r14,%r15
mov    0xc8(%rsp),%rsi
add    %r14,%rsi
add    $0x6,%r15
add    %rax,%rax
lea    (%rax,%rax,2),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ba2>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
shl    $0x6,%eax
lea    (%rax,%r8,1),%rsi
add    $0xc0,%rsi
xor    %eax,%eax
cmp    $0x18,%r14
setae  %al
lea    0x18(,%rax,8),%rbp
mov    %r10,0x50(%rsp)
lea    (%r10,%rbp,8),%r15
mov    0x38(%rsp),%r13
test   %r13,%r13
mov    %rsi,0xc8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b4c>
lea    0x0(,%r13,8),%r14
sub    %r13,%r14
mov    %r15,%rdi
mov    %r14,%rdx
call   *0x16d92b(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4b4f>
xor    %r14d,%r14d
mov    0x40(%rsp),%rbp
mov    0x30(%rsp),%rsi
mov    %rsi,%rcx
shr    $0x20,%rcx
mov    %rsi,%rdx
shr    $0x30,%rdx
mov    0x60(%rsp),%rax
mov    %esi,(%r15,%r14,1)
mov    %cx,0x4(%r15,%r14,1)
mov    %dl,0x6(%r15,%r14,1)
sub    %r13,%rax
mov    0x50(%rsp),%r10
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4bb6>
add    %r14,%r15
mov    0xc8(%rsp),%rsi
add    %r14,%rsi
add    $0x7,%r15
lea    0x0(,%rax,8),%rdx
sub    %rax,%rdx
mov    %r15,%rdi
mov    %r10,%r14
call   *0x16d8c2(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r14,%r10
mov    0x18(%rsp),%r8
mov    0x10(%rsp),%rdx
mov    %r10,(%rdx)
mov    0x90(%rsp),%eax
mov    0x93(%rsp),%ecx
mov    0x58(%rsp),%rsi
mov    %ecx,0x3(%rsi)
mov    %rdx,%rcx
mov    %eax,(%rsi)
mov    0x8(%rdx),%rax
mov    0x60(%rsp),%rdx
xor    %rax,%rdx
and    0x48(%rsp),%rdx
xor    %rax,%rdx
mov    %rdx,0x8(%rcx)
test   %r8,%r8
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x639f>
mov    0x70(%rsp),%rsi
lea    0xf(%rsi),%rax
and    $0xfffffffffffffff0,%rax
mov    0x20(%rsp),%rdx
lock sub %rax,0x200(%rdx)
lock decq 0x208(%rdx)
cmp    $0x177,%rsi
mov    0x78(%rsp),%r14
mov    0x38(%rsp),%r15
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c61>
lea    -0x1fb68d(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rsi,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c61>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x662e>
mov    0x20(%rsp),%rdx
mov    0x10(%rdx,%rdi,8),%rax
mov    %rax,(%r8)
mov    %r8,0x10(%rdx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4c75>
mov    %r8,%rdi
mov    %r10,%r13
call   *0x16d81b(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %r13,%r10
mov    0x10(%rsp),%rcx
movzbl 0xf8(%rsp),%edx
cmpb   $0x1,0x28(%rsp)
sete   %al
test   %al,%dl
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4cdd>
shr    $0x8,%rbx
mov    %rbx,0x40(%r12)
movq   $0x0,0x58(%r12)
mov    %r10,0x60(%r12)
mov    0xd8(%rsp),%rax
mov    %ax,0x70(%r12)
mov    %rcx,(%r12)
movb   $0x1,0x48(%r12)
movq   $0x1,0x50(%r12)
movq   $0x0,0x68(%r12)
mov    $0x1,%edx
test   %r14,%r14
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ef0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f80>
mov    0x0(%rbp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ee5>
mov    0x68(%r12),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ec3>
movq   $0x0,0x68(%r12)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ec3>
movzbl 0x49(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d41>
mov    0x8(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fd89f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ec3>
movzbl 0x4a(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4d81>
mov    0x10(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fd8df(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ec3>
movzbl 0x4b(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4dc1>
mov    0x18(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fd91f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ec3>
movzbl 0x4c(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e01>
mov    0x20(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fd95f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ec3>
movzbl 0x4d(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e41>
mov    0x28(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fd99f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ec3>
movzbl 0x4e(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4e7d>
mov    0x30(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fd9db(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4ec3>
movzbl 0x4f(%r12),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4eb9>
mov    0x38(%r12),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fda17(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
movq   $0xffffffffffffffff,0x40(%r12)
movw   $0x0,0x70(%r12)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
xor    %edx,%edx
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f80>
shl    $0x4,%r14
lea    (%rsp,%r14,1),%rax
add    $0x108,%rax
lea    0x108(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f19>
nopl   0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f80>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f4f>
mov    0x8(%rdi),%rdx
lea    -0x1fdaa9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f10>
mov    %rdi,(%r12,%rdx,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6327>
mov    %sil,0x48(%r12,%rdi,1)
mov    0x50(%r12),%rdx
inc    %rdx
mov    %rdx,0x50(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f10>
lea    (%r10,%r15,8),%rax
mov    0x98(%rsp),%rcx
movq   $0x0,(%rcx)
mov    %rax,0x10(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52ec>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x655d>
mov    0x20(%rsp),%r8
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
jg     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4fde>
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
mov    0x58(%rsp),%rsi
mov    %ecx,0x3(%rsi)
mov    %eax,(%rsi)
movb   $0x1,0xf(%rdx)
movzbl %r15b,%r13d
cmp    %r12b,%r15b
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5095>
cmpq   $0x0,0xf0(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66c4>
mov    0xe8(%rsp),%rax
mov    (%rax),%rcx
mov    0x10(%rsp),%rdi
mov    %r13d,%esi
mov    0x50(%rsp),%edx
call   *0x16d75b(%rip)        # ADDR <_DYNAMIC+0x598>
mov    0xf0(%rsp),%r12
test   %r12,%r12
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5103>
mov    %r13,0x38(%rsp)
mov    0xe8(%rsp),%r13
shl    $0x4,%r12
add    %r13,%r12
lea    0xa8(%rsp),%r15
xchg   %ax,%ax
mov    0x0(%r13),%rcx
mov    0x8(%r13),%r8
mov    0x1c0(%rsp),%rax
mov    %rax,(%rsp)
mov    %r15,%rdi
mov    0x20(%rsp),%rsi
mov    0x10(%rsp),%rdx
mov    0x50(%rsp),%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
add    $0x10,%r13
cmp    %r12,%r13
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x50c0>
mov    0xf0(%rsp),%rax
mov    0x38(%rsp),%r13
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5105>
xor    %eax,%eax
cmp    $0x8,%r13b
mov    0x1c0(%rsp),%r12
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66b3>
dec    %rax
mov    0x58(%rsp),%rsi
mov    (%rsi),%rcx
xor    %rcx,%rax
lea    -0x1fdca4(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rdx,%r13,8),%rax
xor    %rcx,%rax
mov    %rax,(%rsi)
cmpq   $0x0,0x60(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63cf>
lea    0x8(%r14),%rax
imul   %rax,%rbp
lea    0xc(%rbp),%rax
and    $0xfffffffffffffff0,%rax
mov    0x20(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
cmp    $0x177,%rbp
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x51b5>
lea    -0x1fbbce(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl 0x0(%rbp,%rax,1),%edi
cmp    $0xff,%rdi
mov    0x78(%rsp),%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x51cd>
cmp    $0x3e,%dil
mov    0x40(%rsp),%r11
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x666c>
mov    0x10(%rcx,%rdi,8),%rax
mov    0x60(%rsp),%rdx
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
test   %r14,%r14
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x51e6>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5277>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%rbp
mov    0x78(%rsp),%r14
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x641b>
mov    0x60(%rsp),%rdi
call   *0x16d2b0(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x40(%rsp),%r11
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5277>
shl    $0x4,%r14
lea    (%rsp,%r14,1),%rax
add    $0x108,%rax
lea    0x108(%rsp),%rcx
lea    0x16a8c5(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5219>
data16 cs nopw 0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5277>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x524b>
mov    0x8(%rdi),%r8
lea    -0x1fdda9(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5210>
mov    %rdi,(%r12,%r8,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65f4>
mov    %sil,0x48(%r12,%rdi,1)
incq   0x50(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5210>
mov    0x16d4c2(%rip),%rax        # ADDR <_DYNAMIC+0x4e8>
movzbl (%rax),%eax
cmp    $0x2,%al
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5299>
movzbl 0x28(%rsp),%edx
mov    0x10(%rsp),%rdi
mov    %rbx,%rsi
call   ADDR <expanse_trie::get::locate_slot_popcnt>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52b7>
movzbl %al,%eax
cmp    $0x1,%eax
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6448>
movzbl 0x28(%rsp),%edx
mov    0x10(%rsp),%rdi
mov    %rbx,%rsi
call   ADDR <expanse_trie::get::locate_slot_swar>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63e4>
mov    0x98(%rsp),%rcx
movq   $0x0,(%rcx)
mov    %rax,0x10(%rcx)
cmpq   $0x0,0xe0(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52ec>
mov    0xe8(%rsp),%rdi
call   *0x16d19c(%rip)        # ADDR <free@GLIBC_2.2.5>
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
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5373>
lea    (%r10,%r10,2),%rax
mov    %bx,0x0(%r13,%rax,1)
shr    $0x10,%ebx
mov    %bl,0x2(%r13,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5373>
mov    %ebx,0x0(%r13,%r10,4)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5373>
mov    %rbx,0x0(%r13,%r10,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5373>
mov    %bx,0x0(%r13,%r10,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5373>
lea    (%r10,%r10,2),%rax
mov    %ebx,0x0(%r13,%rax,2)
shr    $0x20,%rbx
mov    %bx,0x4(%r13,%rax,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5373>
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
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x53d3>
mov    0x18(%rsp),%rax
lea    (%rax,%r10,8),%rsi
lea    0x1(%r10),%r12
lea    (%r15,%r10,8),%rdi
add    $0x8,%rdi
lea    0x0(,%rbx,8),%edx
call   *0x16d0d3(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x58(%rsp),%rsi
imul   %r14,%rsi
add    0xa0(%rsp),%rsi
imul   %r14,%r12
add    %r12,%r13
imul   %r14,%rbx
mov    %r13,%rdi
mov    %rbx,%rdx
call   *0x16d0ab(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x28(%rsp),%rdx
mov    0x58(%rsp),%r10
lea    0x1(%rdx),%r8
mov    0x10(%rsp),%rax
mov    (%rax),%rdi
test   %rdi,%rdi
mov    0x1c0(%rsp),%rbx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6488>
lea    0x0(,%r8,8),%eax
add    $0x18,%eax
and    $0xffffffe0,%eax
cmp    $0x1,%dl
mov    $0x10,%ecx
cmovne %rax,%rcx
mov    0x20(%rsp),%rax
lock sub %rcx,0x200(%rax)
lock decq 0x208(%rax)
lea    -0x1fbe79(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rax,1),%eax
cmp    $0xff,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x544a>
cmp    $0x3e,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x667d>
mov    0x20(%rsp),%rdx
mov    0x10(%rdx,%rax,8),%rcx
mov    %rcx,(%rdi)
mov    %rdi,0x10(%rdx,%rax,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x545a>
call   *0x16d038(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x58(%rsp),%r10
mov    0x30(%rsp),%r8
mov    0xa0(%rsp),%rdi
lea    0x4(%r14),%eax
movl   $0x0,0xab(%rsp)
movl   $0x0,0xa8(%rsp)
mov    0x10(%rsp),%rsi
mov    %r15,(%rsi)
mov    0xa8(%rsp),%ecx
mov    0xab(%rsp),%edx
mov    %edx,0x3(%rdi)
mov    %ecx,(%rdi)
mov    %al,0xf(%rsi)
mov    0x8(%rsi),%rax
xor    %rax,%r8
lea    -0x1fe020(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
and    (%rcx,%r14,8),%r8
xor    %rax,%r8
mov    %r8,0x8(%rsi)
mov    0x78(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5545>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x108,%rax
lea    0x108(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54e9>
nopl   0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5545>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x551b>
mov    0x8(%rsi),%rdi
lea    -0x1fe079(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54e0>
mov    %rsi,(%rbx,%rdi,8)
mov    0x50(%rbx),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6327>
mov    %dl,0x48(%rbx,%rdi,1)
incq   0x50(%rbx)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x54e0>
lea    (%r15,%r10,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f84>
mov    %r10,0x60(%rsp)
mov    %rdx,0xd8(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x645b>
mov    0x20(%rsp),%rcx
mov    0x220(%rcx),%rdx
mov    %rdx,(%rax)
mov    %rbp,0x8(%rax)
mov    %rax,0x48(%rsp)
mov    %rax,0x220(%rcx)
mov    $0xfc0,%ax
xor    %edx,%edx
divw   0xd8(%rsp)
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
mov    0x60(%rsp),%r10
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x55f6>
mov    %r10d,%eax
and    $0xffffffe0,%eax
lea    -0x1(%rcx),%rsi
imul   %rax,%rsi
mov    0x48(%rsp),%rdi
add    %rdi,%rsi
add    $0x40,%rsi
neg    %rax
xor    %edi,%edi
mov    0x20(%rsp),%r9
mov    0x10(%r9,%rbp,8),%r8
mov    %r8,(%rsi)
mov    %rsi,0x10(%r9,%rbp,8)
inc    %rdi
add    %rax,%rsi
cmp    %rdi,%rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x55d8>
mov    %rcx,%rax
sub    %rdi,%rax
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
mov    0x20(%rsp),%rdx
jb     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x56b6>
lea    -0x4(%rax),%rcx
shr    $0x5,%r10d
imul   %r10,%rcx
shl    $0x5,%rcx
add    $0x40,%rcx
mov    %r10d,%esi
shl    $0x7,%esi
neg    %rsi
mov    %rsi,0x60(%rsp)
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
mov    0x48(%rsp),%r9
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
add    0x60(%rsp),%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x565f>
mov    0x48(%rsp),%rdi
add    $0x40,%rdi
mov    0xd8(%rsp),%rdx
mov    %rdi,0x48(%rsp)
xor    %esi,%esi
call   *0x16ce8c(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x58(%rsp),%rcx
test   %rcx,%rcx
mov    0x18(%rsp),%rsi
mov    0x48(%rsp),%rbp
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5703>
lea    0x0(,%rcx,8),%rdx
mov    %rbp,%rdi
call   *0x16cd77(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%rsi
mov    0x58(%rsp),%rcx
lea    0x0(,%rcx,8),%rdx
add    %rbp,%rdx
mov    0x50(%rsp),%rax
mov    %rax,0x0(%rbp,%rcx,8)
mov    %rdx,%rbp
mov    0x28(%rsp),%r8
lea    0x1(%r8),%rdx
cmp    %rcx,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5741>
lea    (%rsi,%rcx,8),%rsi
lea    0x8(%rbp),%rdi
sub    %ecx,%edx
shl    $0x3,%edx
call   *0x16cd34(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x28(%rsp),%r8
mov    0x10(%rsp),%rax
mov    (%rax),%rdi
test   %rdi,%rdi
mov    0x20(%rsp),%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64e1>
mov    0xf8(%rsp),%rdx
add    $0x18,%edx
and    $0xffffffe0,%edx
cmp    $0x1,%r8b
mov    $0x10,%eax
cmovne %rdx,%rax
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
lea    -0x1fc1e0(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rax,%rdx,1),%eax
cmp    $0xff,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57af>
cmp    $0x3e,%al
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x667d>
mov    %rcx,%rdx
mov    0x10(%rcx,%rax,8),%rcx
mov    %rcx,(%rdi)
mov    %rdi,0x10(%rdx,%rax,8)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x57b5>
call   *0x16ccd3(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0xa0(%rsp),%rsi
mov    0x30(%rsp),%rdx
mov    0x58(%rsp),%rdi
mov    (%rsi),%eax
mov    0x3(%rsi),%ecx
mov    %ecx,0x8b(%rsp)
mov    %eax,0x88(%rsp)
cmp    %rdi,%rdx
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5817>
mov    %rdi,%rdx
imul   %r14,%rdx
lea    0x1(%rdi),%r8
imul   %r14,%r8
lea    0x1638f3(%rip),%r9        # ADDR <anon.ADDR.2.llvm.ADDR+0x958>
lea    0x88(%rsp),%rdi
mov    $0x7,%esi
mov    %r15,%rcx
call   ADDR <<[u8]>::copy_within::<core::ops::range::Range<usize>>>
mov    0x58(%rsp),%rdi
mov    0xa0(%rsp),%rsi
mov    0x38(%rsp),%rcx
dec    %ecx
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e2b>
lea    -0x2093b2(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0x1889>
movslq (%rax,%rcx,4),%rcx
add    %rax,%rcx
jmp    *%rcx
mov    %bl,0x88(%rsp,%rdi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e2b>
mov    %r8,%r13
sub    %r15,%r13
mov    0x40(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f24>
lea    (%r11,%r15,8),%rsi
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %rdx,0x30(%rsp)
lea    0x0(,%r13,8),%rdx
mov    %r10,0x48(%rsp)
mov    %r8,0x70(%rsp)
call   *0x16ccc3(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    (%r15,%r15,4),%r15
lea    (%r12,%r15,1),%rsi
lea    (%r12,%r15,1),%rdi
add    $0x5,%rdi
lea    0x0(,%r13,4),%rdx
add    %r13,%rdx
call   *0x16cca2(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x38(%rsp),%rsi
mov    0x70(%rsp),%r8
mov    0x30(%rsp),%rdx
mov    0x48(%rsp),%r10
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f2b>
mov    %r8,%r13
sub    %r15,%r13
mov    0x40(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f4b>
lea    (%r11,%r15,8),%rsi
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %rdx,0x30(%rsp)
lea    0x0(,%r13,8),%rdx
mov    %r10,0x48(%rsp)
mov    %r8,0x70(%rsp)
call   *0x16cc4a(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    (%r15,%r15,2),%r15
lea    (%r12,%r15,1),%rsi
lea    (%r12,%r15,1),%rdi
add    $0x3,%rdi
lea    0x0(,%r13,2),%rdx
add    %r13,%rdx
call   *0x16cc29(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x38(%rsp),%rsi
mov    0x70(%rsp),%r8
mov    0x30(%rsp),%rdx
mov    0x48(%rsp),%r10
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f52>
mov    %r8,%r13
sub    %r15,%r13
mov    0x40(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f6f>
lea    (%r11,%r15,8),%rsi
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %rdx,0x30(%rsp)
lea    0x0(,%r13,8),%rdx
mov    %r10,0x48(%rsp)
mov    %r8,0x70(%rsp)
call   *0x16cbd1(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    (%r12,%r15,4),%rsi
lea    (%r12,%r15,4),%rdi
add    $0x4,%rdi
shl    $0x2,%r13
mov    %r13,%rdx
call   *0x16cbb8(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    0x0(,%r15,4),%rsi
mov    0x70(%rsp),%r8
mov    0x30(%rsp),%rdx
mov    0x48(%rsp),%r10
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f77>
mov    %r8,%r13
sub    %r15,%r13
mov    0x40(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f8b>
lea    (%r11,%r15,8),%rsi
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %rdx,0x30(%rsp)
lea    0x0(,%r13,8),%rdx
mov    %r10,0x48(%rsp)
mov    %r8,0x70(%rsp)
call   *0x16cb5d(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    (%r12,%r15,2),%rsi
lea    (%r12,%r15,2),%rdi
add    $0x2,%rdi
add    %r13,%r13
mov    %r13,%rdx
call   *0x16cb45(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    (%r15,%r15,1),%rsi
mov    0x70(%rsp),%r8
mov    0x30(%rsp),%rdx
mov    0x48(%rsp),%r10
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f8f>
mov    %r8,%r13
sub    %r15,%r13
mov    0x40(%rsp),%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5fa4>
lea    (%r11,%r15,8),%rsi
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
mov    %rdx,0x30(%rsp)
lea    0x0(,%r13,8),%rdx
mov    %r10,0x48(%rsp)
mov    %r8,0x70(%rsp)
call   *0x16caee(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    (%r15,%r15,1),%rax
lea    (%rax,%rax,2),%r15
lea    (%r12,%r15,1),%rsi
lea    (%r12,%r15,1),%rdi
add    $0x6,%rdi
add    %r13,%r13
lea    0x0(,%r13,2),%rdx
add    %r13,%rdx
call   *0x16cac6(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x38(%rsp),%rsi
mov    0x70(%rsp),%r8
mov    0x30(%rsp),%rdx
mov    0x48(%rsp),%r10
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5faf>
mov    %r8,%rbp
sub    %r15,%rbp
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5fd0>
lea    (%r11,%r15,8),%rsi
lea    0x0(,%r15,8),%r13
lea    (%r11,%r15,8),%rdi
add    $0x8,%rdi
lea    0x0(,%rbp,8),%r15
mov    %rdx,0x30(%rsp)
mov    %r15,%rdx
mov    %r10,0x48(%rsp)
mov    %r8,0x70(%rsp)
call   *0x16ca68(%rip)        # ADDR <memmove@GLIBC_2.2.5>
sub    0x38(%rsp),%r13
lea    (%r12,%r13,1),%rsi
lea    (%r12,%r13,1),%rdi
add    $0x7,%rdi
sub    %rbp,%r15
mov    %r15,%rdx
mov    0x38(%rsp),%r15
call   *0x16ca46(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x70(%rsp),%r8
mov    0x30(%rsp),%rdx
mov    0x48(%rsp),%r10
mov    0x18(%rsp),%r11
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5fdb>
mov    %r15d,0x68(%rsp)
shr    $0x20,%r15
mov    %r15b,0x6c(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b9c>
mov    %r15w,0x68(%rsp)
shr    $0x10,%r15d
mov    %r15b,0x6a(%rsp)
mov    %bp,0x6b(%rsp)
shr    $0x10,%ebp
mov    %bpl,0x6d(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b9c>
mov    %r15d,0x68(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b9c>
xor    %r15d,%r15d
mov    0x1c0(%rsp),%r11
mov    0x78(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b9c>
mov    %r15w,0x68(%rsp)
mov    %bp,0x6a(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b9c>
mov    %r15d,0x68(%rsp)
shr    $0x20,%r15
mov    %r15w,0x6c(%rsp)
xor    %r15d,%r15d
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5b9c>
mov    %r15d,0x68(%rsp)
mov    %r15,%rcx
shr    $0x20,%rcx
mov    %cx,0x6c(%rsp)
shr    $0x30,%r15
mov    0x10(%rsp),%rsi
mov    %rax,(%rsi)
movzwl 0x6c(%rsp),%ecx
mov    %cx,0x4(%rdi)
mov    0x68(%rsp),%ecx
mov    %ecx,(%rdi)
mov    %r15b,0xe(%rsi)
lea    0x1(%r13),%ecx
mov    %cl,0xf(%rsi)
test   %rdx,%rdx
mov    0x40(%rsp),%r14
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c45>
shl    $0x4,%rdx
lea    (%rsp,%rdx,1),%rcx
add    $0x108,%rcx
lea    0x108(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5bf3>
mov    %sil,0x48(%r11,%rdi,1)
incq   0x50(%r11)
add    $0x10,%rdx
cmp    %rcx,%rdx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c45>
mov    (%rdx),%rdi
movzbl 0x8(%rdx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5c25>
mov    0x8(%rdi),%r8
lea    -0x1fe783(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5bea>
mov    %rdi,(%r11,%r8,8)
mov    0x50(%r11),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5be1>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6327>
lea    (%rax,%rbx,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f84>
lea    (%r11,%r11,4),%rax
mov    %ebx,0x80(%rsp,%rax,1)
shr    $0x20,%rbx
mov    %bl,0x84(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ccd>
lea    (%r11,%r11,2),%rax
mov    %bx,0x80(%rsp,%rax,1)
shr    $0x10,%ebx
mov    %bl,0x82(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ccd>
mov    %ebx,0x80(%rsp,%r11,4)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ccd>
mov    %bx,0x80(%rsp,%r11,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ccd>
lea    (%r11,%r11,2),%rax
mov    %ebx,0x80(%rsp,%rax,2)
shr    $0x20,%rbx
mov    %bx,0x84(%rsp,%rax,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ccd>
lea    (%rsp,%r11,8),%rax
add    $0x80,%rax
sub    %r11,%rax
mov    %ebx,(%rax)
mov    %rbx,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%rbx
mov    %bl,0x6(%rax)
imul   %r9d,%r14d
cmp    $0x10,%r14d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64c8>
mov    0x80(%rsp),%eax
mov    0x83(%rsp),%ecx
mov    %ecx,0x3(%r12)
mov    %eax,(%r12)
and    $0x70,%r13b
dec    %r9b
or     %r13b,%r9b
mov    0x10(%rsp),%rax
mov    %r9b,0xf(%rax)
mov    0x78(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d96>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x108,%rax
lea    0x108(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d3c>
mov    %dl,0x48(%r8,%rdi,1)
incq   0x50(%r8)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d96>
mov    (%rcx),%r11
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d6e>
mov    0x8(%r11),%rdi
lea    -0x1fe8cc(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d33>
mov    0x1c0(%rsp),%r8
mov    %r11,(%r8,%rdi,8)
mov    0x50(%r8),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5d2a>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6327>
mov    0x98(%rsp),%rax
movq   $0x0,(%rax)
mov    %rsi,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52ec>
lea    (%rdi,%rdi,4),%rax
mov    %ebx,0x88(%rsp,%rax,1)
shr    $0x20,%rbx
mov    %bl,0x8c(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e2b>
lea    (%rdi,%rdi,2),%rax
mov    %bx,0x88(%rsp,%rax,1)
shr    $0x10,%ebx
mov    %bl,0x8a(%rsp,%rax,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e2b>
mov    %ebx,0x88(%rsp,%rdi,4)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e2b>
mov    %bx,0x88(%rsp,%rdi,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e2b>
lea    (%rdi,%rdi,2),%rax
mov    %ebx,0x88(%rsp,%rax,2)
shr    $0x20,%rbx
mov    %bx,0x8c(%rsp,%rax,2)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5e2b>
lea    (%rsp,%rdi,8),%rax
add    $0x88,%rax
sub    %rdi,%rax
mov    %ebx,(%rax)
mov    %rbx,%rcx
shr    $0x20,%rcx
mov    %cx,0x4(%rax)
shr    $0x30,%rbx
mov    %bl,0x6(%rax)
mov    0x70(%rsp),%rdi
imul   %edi,%r14d
cmp    $0x10,%r14d
jae    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x64fa>
mov    0x10(%rsp),%rdx
mov    0x48(%rsp),%rax
mov    %rax,(%rdx)
mov    0x88(%rsp),%eax
mov    0x8b(%rsp),%ecx
mov    %ecx,0x3(%rsi)
mov    %eax,(%rsi)
and    $0x70,%r13b
dec    %dil
or     %r13b,%dil
mov    %dil,0xf(%rdx)
mov    0x78(%rsp),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f0c>
shl    $0x4,%rax
add    %rsp,%rax
add    $0x108,%rax
lea    0x108(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ea9>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5f0c>
mov    (%rcx),%rsi
movzbl 0x8(%rcx),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5edb>
mov    0x8(%rsi),%rdi
lea    -0x1fea39(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%r8
mov    %r8,%r9
and    %rdi,%r9
inc    %r9
xor    %rdi,%r9
and    %r8,%r9
xor    %rdi,%r9
mov    %r9,0x8(%rsi)
mov    0x40(%rsp),%rdi
mov    (%rdi),%rdi
lea    -0x1(%rdi),%r8
cmp    $0x6,%r8
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ea0>
mov    %rsi,(%r12,%rdi,8)
mov    0x50(%r12),%rdi
cmp    $0x7,%rdi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6327>
mov    %dl,0x48(%r12,%rdi,1)
incq   0x50(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5ea0>
mov    0x98(%rsp),%rax
movq   $0x0,(%rax)
mov    %rbp,0x10(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52ec>
mov    %r15,%rsi
lea    (%r15,%r15,4),%r15
mov    0x10(%rsp),%rcx
mov    0x50(%rsp),%rax
mov    %rax,(%r11,%rsi,8)
mov    %edx,(%r12,%r15,1)
shr    $0x20,%rdx
mov    %dl,0x4(%r12,%r15,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5fcb>
mov    %r15,%rsi
lea    (%r15,%r15,2),%r15
mov    0x10(%rsp),%rcx
mov    0x50(%rsp),%rax
mov    %rax,(%r11,%rsi,8)
mov    %dx,(%r12,%r15,1)
shr    $0x10,%edx
mov    %dl,0x2(%r12,%r15,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5fcb>
lea    0x0(,%r15,4),%rsi
mov    0x10(%rsp),%rcx
mov    0x50(%rsp),%rax
mov    %rax,(%r11,%r15,8)
mov    %edx,(%r12,%rsi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6008>
lea    (%r15,%r15,1),%rsi
mov    0x10(%rsp),%rcx
mov    0x50(%rsp),%rax
mov    %rax,(%r11,%r15,8)
mov    %dx,(%r12,%rsi,1)
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6008>
lea    (%r15,%r15,1),%rax
mov    %r15,%rsi
lea    (%rax,%rax,2),%r15
mov    0x10(%rsp),%rcx
mov    0x50(%rsp),%rax
mov    %rax,(%r11,%rsi,8)
mov    %edx,(%r12,%r15,1)
shr    $0x20,%rdx
mov    %dx,0x4(%r12,%r15,1)
mov    %rsi,%r15
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6008>
lea    0x0(,%r15,8),%r13
sub    %r15,%r13
mov    0x40(%rsp),%rbp
mov    0x10(%rsp),%rcx
mov    0x50(%rsp),%rax
mov    %rax,(%r11,%r15,8)
mov    %edx,(%r12,%r13,1)
mov    %rdx,%rax
shr    $0x20,%rax
mov    %ax,0x4(%r12,%r13,1)
shr    $0x30,%rdx
mov    %dl,0x6(%r12,%r13,1)
mov    0x58(%rsp),%rdx
mov    (%rdx),%rax
xor    %rax,%r8
and    %r10,%r8
xor    %rax,%r8
mov    %r8,(%rdx)
cmp    $0x1,%r14d
mov    0x1c0(%rsp),%r10
mov    0x28(%rsp),%rax
mov    0xd8(%rsp),%rdx
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x607b>
cmp    $0x1,%al
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x607b>
shr    $0x8,%rbx
mov    %rbx,0x40(%r10)
movq   $0x0,0x58(%r10)
mov    %r11,0x60(%r10)
mov    %dx,0x70(%r10)
mov    %rcx,(%r10)
movb   $0x1,0x48(%r10)
movq   $0x1,0x50(%r10)
movq   $0x0,0x68(%r10)
mov    $0x1,%edx
mov    0x78(%rsp),%r9
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6298>
mov    0x0(%rbp),%rax
test   %rax,%rax
mov    0x78(%rsp),%r9
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6296>
mov    0x68(%r10),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6276>
movq   $0x0,0x68(%r10)
cmp    $0x1,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6276>
movzbl 0x49(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x60e0>
mov    0x8(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fec3e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6276>
movzbl 0x4a(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x611e>
mov    0x10(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fec7c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6276>
movzbl 0x4b(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x615c>
mov    0x18(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fecba(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6276>
movzbl 0x4c(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x619a>
mov    0x20(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fecf8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6276>
movzbl 0x4d(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x61d8>
mov    0x28(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fed36(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6276>
movzbl 0x4e(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6216>
mov    0x30(%r10),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fed74(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6276>
movzbl 0x4f(%r10),%edx
cmp    $0x7,%rdx
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6258>
mov    0x1c0(%rsp),%rsi
mov    0x38(%rsi),%rsi
mov    0x8(%rsi),%rdi
lea    -0x1fedb6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
mov    0x40(%rsp),%rbp
mov    0x78(%rsp),%r9
mov    0x38(%rsp),%r15
mov    0x18(%rsp),%r11
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6640>
movq   $0xffffffffffffffff,0x40(%r10)
movw   $0x0,0x70(%r10)
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%rbp)
movq   $0x0,0x10(%rbp)
xor    %edx,%edx
test   %r9,%r9
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6339>
shl    $0x4,%r9
lea    (%rsp,%r9,1),%rax
add    $0x108,%rax
lea    0x108(%rsp),%rcx
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62d9>
nopl   0x0(%rax)
mov    %sil,0x48(%r10,%rdi,1)
mov    0x50(%r10),%rdx
inc    %rdx
mov    %rdx,0x50(%r10)
add    $0x10,%rcx
cmp    %rax,%rcx
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6339>
mov    (%rcx),%rdi
movzbl 0x8(%rcx),%esi
cmp    $0x7,%rsi
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x630f>
mov    0x8(%rdi),%rdx
lea    -0x1fee69(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62d0>
mov    %rdi,(%r10,%rdx,8)
mov    0x50(%r10),%rdi
cmp    $0x7,%rdi
jbe    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x62c0>
lea    0x16979a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7338>
mov    $0x8,%esi
call   *0x16c127(%rip)        # ADDR <_DYNAMIC+0x208>
lea    (%r11,%r15,8),%rax
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x4f84>
lea    -0x1fee7b(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8db>
lea    0x162d50(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x910>
mov    $0xa1,%esi
call   *0x16c175(%rip)        # ADDR <_DYNAMIC+0x278>
lea    0x162f66(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xb38>
mov    $0x7,%esi
call   *0x16c0f3(%rip)        # ADDR <_DYNAMIC+0x208>
lea    -0x1fee19(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc968>
lea    0x162d3d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x928>
mov    $0x13,%esi
call   *0x16c2a2(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x222565(%rip),%rdi        # ADDR <anon.ADDR.91.llvm.ADDR+0x260>
lea    0x162c94(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x898>
mov    $0x8,%esi
call   *0x16c289(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x222546(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
lea    0x162e2b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa48>
mov    $0x8,%esi
call   *0x16c270(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x162da1(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x9d0>
mov    0x38(%rsp),%rdi
call   *0x16c12e(%rip)        # ADDR <_DYNAMIC+0x2a0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66d5>
lea    0x162dea(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa30>
mov    $0x8,%esi
lea    -0x222582(%rip),%rdi        # ADDR <anon.ADDR.751.llvm.ADDR+0x20>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x63f7>
lea    0x162dbd(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa18>
mov    $0x11,%esi
lea    -0x1feead(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc957>
call   *0x16c22b(%rip)        # ADDR <_DYNAMIC+0x3d0>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66d5>
lea    -0x1fef41(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8d5>
lea    0x162c78(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8f8>
mov    $0x6,%esi
call   *0x16c20d(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x1ff02d(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0x169407(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x162590(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0xd7(%rsp),%rdx
mov    $0x11,%esi
call   *0x16c0dd(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66d5>
call   *0x16c2fa(%rip)        # ADDR <_DYNAMIC+0x4f0>
test   %al,%al
jne    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x5285>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x52a5>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0x16c06d(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x16155e(%rip),%rcx        # ADDR <anon.ADDR.892.llvm.ADDR>
mov    %rdx,%rsi
mov    $0x7,%edx
mov    %r14,%rdi
call   *0x16c025(%rip)        # ADDR <_DYNAMIC+0x250>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66d5>
lea    -0x1fef5f(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc93d>
lea    0x162c3a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x940>
mov    $0xb,%esi
call   *0x16c187(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x1626e8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
call   *0x16bfad(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1626d6(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
mov    %rax,%rdi
call   *0x16bf98(%rip)        # ADDR <_DYNAMIC+0x208>
lea    -0x1fefb1(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x162c72(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x9b8>
mov    $0x12,%esi
call   *0x16c147(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x1fefb8(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc93d>
lea    0x162c29(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x988>
mov    $0xb,%esi
call   *0x16c12e(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x1fefe3(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x162bf8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x970>
mov    $0x12,%esi
call   *0x16c115(%rip)        # ADDR <_DYNAMIC+0x3d0>
mov    $0x10,%edi
mov    %r13,%rsi
call   *0x16bfb7(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x1626c8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x460>
mov    $0x8,%esi
call   *0x16bf2d(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162b36(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8e0>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16bf18(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162b09(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8c8>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16bf03(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0x16bf6b(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66d5>
lea    0x162a1f(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x808>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16bed9(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162a3a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x838>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16bec4(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162a3d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x850>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0x16bead(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1629f6(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x820>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16be98(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162a29(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x868>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16be83(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162a2c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x880>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0x16be6c(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x8,%esi
call   *0x16be61(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66d5>
lea    0x162a35(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x8b0>
mov    $0x8,%esi
mov    %r14,%rdi
call   *0x16be47(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1623b8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %rbp,%rdi
call   *0x16be32(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162373(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
call   *0x16be20(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x169499(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0x16be09(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x16237a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r15,%rdi
call   *0x16bdf4(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x162335(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x65f9>
lea    0x162324(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
mov    %rax,%rdi
call   *0x16bdce(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x10,%edi
mov    %r14,%rsi
call   *0x16be38(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x162ad1(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x9e8>
xor    %edi,%edi
xor    %esi,%esi
call   *0x16bdaf(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66d5>
mov    %r13,%rdi
lea    0x1625ab(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4d8>
mov    $0x8,%esi
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66cf>
lea    0x162ac5(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0xa00>
xor    %edi,%edi
xor    %esi,%esi
call   *0x16bd8b(%rip)        # ADDR <_DYNAMIC+0x208>
ud2
mov    %rax,%rbx
cmpq   $0x0,0xa8(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66fa>
mov    0xb0(%rsp),%rdi
call   *0x16bd95(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66fa>
jmp    ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x66f7>
mov    %rax,%rbx
cmpq   $0x0,0xe0(%rsp)
je     ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>+0x6713>
mov    0xe8(%rsp),%rdi
call   *0x16bd75(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
int3
int3
int3
int3
int3
