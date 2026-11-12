push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x4a8,%rsp
mov    %ecx,%r14d
mov    %rsi,%r12
movzbl 0xf(%rsi),%r9d
movzwl %r9w,%eax
cmp    $0xd,%r9w
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe4>
mov    %eax,%ecx
lea    -0x1f1420(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xa11>
movslq (%rsi,%rcx,4),%rcx
add    %rsi,%rcx
jmp    *%rcx
mov    %rdi,%r8
mov    (%r12),%rbp
movzbl 0x5(%rbp),%r13d
cmp    %r13b,%r14b
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x86>
sub    %r13b,%r14b
movzbl %r14b,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29f2>
lea    -0x1e55ce(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
lea    0x0(,%r13,8),%ecx
shl    %cl,%rax
mov    0x8(%r12),%rcx
xor    %rdx,%rcx
test   %rax,%rcx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
lea    -0x1(%r13),%esi
lea    0x0(,%rsi,8),%ecx
mov    %rdx,%rdi
shr    %cl,%rdi
mov    %edi,%ecx
and    $0xf,%cl
mov    $0x1,%eax
shl    %cl,%eax
and    0x6(%rbp),%ax
cmp    $0x1,%r9b
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x65b>
test   %ax,%ax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
movzbl 0x4(%rbp),%ecx
lea    0x8(%rbp),%rbx
cmp    $0x4,%cl
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xba7>
test   %cl,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfde>
cmp    %dil,(%rbx)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfde>
xor    %r14d,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x100a>
cmp    $0x7f,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b>
mov    %eax,%esi
shr    $0x4,%esi
and    $0xf,%eax
mov    %rax,%rbx
lea    0x1(%rax),%ecx
mov    %ecx,%eax
mul    %sil
test   %r9b,%r9b
js     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a38>
test   %sil,%sil
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a38>
cmp    $0x10,%al
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a38>
mov    %r12,%r13
movzwl %cx,%eax
shl    $0x8,%eax
movzwl %si,%r15d
or     %eax,%r15d
mov    %r15d,%r12d
shr    $0x8,%r12d
lea    0x0(,%r15,8),%ecx
mov    $0xffffffffffffffff,%rax
shl    %cl,%rax
not    %rax
and    %rax,%rdx
lea    0x8(%r13),%rbp
cmp    $0x1,%r12d
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xa32>
dec    %esi
cmp    $0x6,%esi
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x158b>
lea    -0x1f1339(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc2d>
movslq (%rax,%rsi,4),%rcx
add    %rax,%rcx
jmp    *%rcx
movzbl 0x0(%rbp),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1635>
mov    $0x4,%r15b
mov    %r14d,%eax
sub    %r15b,%al
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x252>
mov    %rdi,%r8
movzbl %al,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29f2>
lea    -0x1e5709(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
lea    0x0(,%r15,8),%ecx
shl    %cl,%rax
mov    0x8(%r12),%rbp
mov    %rbp,%rsi
xor    %rdx,%rsi
test   %rax,%rsi
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
movzbl %cl,%r13d
mov    %r8,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x263>
mov    %r14,%r13
lea    -0x1(%r14),%eax
lea    0x0(,%rax,8),%ecx
mov    %rdx,%rsi
shr    %cl,%rsi
mov    (%r12),%rbx
movzbl %sil,%ecx
shl    $0x4,%ecx
lea    (%rbx,%rcx,1),%r14
add    $0x40,%r14
movzbl %al,%ecx
mov    %rdi,%r15
mov    %r14,%rsi
call   ADDR <expanse_trie::mutate_map::map_remove::<false, false>>
test   $0x1,%al
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    %rdx,%r10
cmpb   $0x0,0xf(%r14)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xc73>
mov    $0x1,%eax
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    0x8(%r12),%rcx
movzbl %r13b,%edx
lea    -0x1e57a2(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rsi,%rdx,8),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x10e8>
mov    $0x7,%r15b
mov    %r14d,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18a>
mov    0x8(%r12),%rbp
lea    0x0(,%r15,8),%eax
movzbl %al,%r13d
movzbl %r15b,%r8d
lea    -0x1e57d6(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%r8,8),%rax
mov    %rax,0x38(%rsp)
and    %rax,%rbp
lea    0x1(%rbp),%rsi
mov    $0xffffffffffffffff,%rax
mov    %r13d,%ecx
shl    %cl,%rax
not    %rax
and    %rax,%rdx
mov    %rdx,%rcx
mov    (%r12),%rdx
mov    %rsi,%rax
cmp    $0x3,%rsi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ca>
cmp    $0x11,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b4>
mov    %rbp,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ca>
mov    $0x18,%eax
cmp    $0x19,%rsi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ca>
mov    $0x20,%eax
cmp    $0x21,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a7>
mov    %r9d,0x10(%rsp)
mov    %rdi,0x8(%rsp)
mov    %rdx,(%rsp)
lea    (%rdx,%rax,8),%rdi
mov    %r8,0x18(%rsp)
mov    %r8d,%edx
call   ADDR <expanse_trie::leaf::locate>
test   $0x1,%al
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x304>
xor    %eax,%eax
mov    %r10,%rdx
add    $0x4a8,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
mov    %rdx,%rbx
test   %r14b,%r14b
mov    0x8(%rsp),%rdx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a2b>
mov    (%rsp),%rsi
mov    (%rsi,%rbx,8),%rcx
mov    $0x7,%al
movzbl %al,%eax
div    %r14b
movzbl %al,%eax
lea    0x1(%rbp),%r9
cmp    %rax,%r9
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x366>
cmp    $0x3,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x68e>
mov    $0x2,%r13d
cmp    $0x3,%r9
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11f2>
mov    $0x1,%r13d
cmp    $0x2,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11f2>
mov    0x18(%rsp),%rax
lea    0x8(%rax),%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x386>
cmp    $0x3,%r9
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x6c6>
mov    0x18(%rsp),%rax
lea    0x8(%rax),%r10
imul   %r9,%r10
test   %rbp,%rbp
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x6d9>
mov    %rcx,%rbx
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12)
lea    0xf(%r10),%rax
and    $0xfffffffffffffff0,%rax
lea    -0x1e37e9(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%r10,%rcx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe93>
sub    %rax,0x200(%rdx)
decq   0x208(%rdx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a51>
mov    0x10(%rdx,%rdi,8),%rax
mov    %rax,(%rsi)
mov    %rsi,0x10(%rdx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xeac>
mov    $0x2,%r15b
mov    %r14d,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18a>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x252>
mov    %rdi,%r8
mov    (%r12),%rbp
mov    %r14,%rax
movzbl 0x74(%rbp),%r14d
cmp    %r14b,%al
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x43d>
sub    %r14b,%al
movzbl %al,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29f2>
lea    -0x1e5985(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
lea    0x0(,%r14,8),%ecx
shl    %cl,%rax
mov    0x8(%r12),%rcx
xor    %rdx,%rcx
test   %rax,%rcx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    %r12,0x30(%rsp)
lea    -0x1(%r14),%eax
lea    0x0(,%rax,8),%ecx
mov    %rdx,%r12
shr    %cl,%r12
mov    %r12d,%ebx
shr    $0x5,%ebx
and    $0x7,%ebx
mov    0x0(%rbp,%rbx,4),%esi
mov    $0x1,%edi
mov    %r12d,%ecx
shl    %cl,%edi
bt     %r12d,%esi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
dec    %edi
and    %edi,%esi
mov    %esi,%ecx
shr    $1,%ecx
and    $0x55555555,%ecx
sub    %ecx,%esi
mov    %esi,%ecx
and    $0x33333333,%ecx
shr    $0x2,%esi
and    $0x33333333,%esi
add    %ecx,%esi
mov    %esi,%ecx
shr    $0x4,%ecx
add    %esi,%ecx
and    $0xf0f0f0f,%ecx
imul   $0x1010101,%ecx,%r15d
shr    $0x18,%r15d
mov    %r15d,%esi
shl    $0x4,%esi
add    0x20(%rbp,%rbx,8),%rsi
movzbl %al,%ecx
mov    %r8,%rdi
mov    %r8,0x8(%rsp)
call   ADDR <expanse_trie::mutate_map::map_remove::<false, false>>
cmp    $0x1,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    %rdx,%r10
mov    0x20(%rbp,%rbx,8),%rsi
mov    %r15d,%edi
shl    $0x4,%edi
cmpb   $0x0,0xf(%rsi,%rdi,1)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x110c>
mov    $0x1,%eax
cmp    $0x7,%r14b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    0x30(%rsp),%rdi
mov    0x8(%rdi),%rcx
lea    -0x1e5a74(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r14,8),%rdx
mov    %rdx,%rsi
and    %rcx,%rsi
dec    %rsi
xor    %rcx,%rsi
and    %rdx,%rsi
xor    %rcx,%rsi
mov    %rsi,0x8(%rdi)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
cmp    $0x1,%r14b
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x568>
mov    %rdi,%rsi
lea    -0x1(%r14),%eax
movzbl %al,%edi
cmp    $0x8,%r14b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29f2>
lea    -0x1e5ab4(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
shl    $0x8,%rax
mov    0x8(%r12),%rcx
xor    %rdx,%rcx
test   %rax,%rcx
mov    %rsi,%rdi
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    (%r12),%r13
mov    %edx,%ebx
shr    $0x5,%ebx
and    $0x7,%ebx
mov    0x0(%r13,%rbx,4),%eax
mov    $0x1,%esi
mov    %edx,%ecx
shl    %cl,%esi
bt     %edx,%eax
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
dec    %esi
and    %eax,%esi
mov    %esi,%ecx
shr    $1,%ecx
and    $0x55555555,%ecx
sub    %ecx,%esi
mov    %esi,%ecx
and    $0x33333333,%ecx
shr    $0x2,%esi
and    $0x33333333,%esi
add    %ecx,%esi
mov    %esi,%ecx
shr    $0x4,%ecx
add    %esi,%ecx
and    $0xf0f0f0f,%ecx
imul   $0x1010101,%ecx,%r9d
shr    $0x18,%r9d
mov    0x20(%r13,%rbx,8),%r8
mov    (%r8,%r9,8),%rcx
mov    %rcx,0x20(%rsp)
lea    -0x1(%rax),%ecx
mov    %eax,%esi
xor    %ecx,%esi
cmp    %ecx,%esi
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xb4d>
movq   $0x0,0x20(%r13,%rbx,8)
addq   $0xfffffffffffffff0,0x200(%rdi)
decq   0x208(%rdi)
mov    0x40(%rdi),%rax
mov    %rax,(%r8)
mov    %r8,0x40(%rdi)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1be1>
mov    $0x1,%r15b
mov    %r14d,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18a>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x252>
mov    $0x5,%r15b
mov    %r14d,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18a>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x252>
mov    $0x3,%r15b
mov    %r14d,%eax
sub    %r15b,%al
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x252>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18a>
mov    $0x6,%r15b
mov    %r14d,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18a>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x252>
test   %ax,%ax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
movzbl 0x4(%rbp),%ecx
lea    0x8(%rbp),%rbx
cmp    $0x4,%cl
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xc0c>
test   %cl,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x105e>
cmp    %dil,(%rbx)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x105e>
xor    %r14d,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x108a>
cmp    $0x11,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xb29>
lea    0x3(%rbp),%r13d
and    $0x3c,%r13d
cmp    $0x11,%r9
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11f2>
mov    %ebp,%eax
and    $0x1c,%eax
add    $0x4,%eax
cmp    %eax,%r13d
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x114f>
lea    0x4(%rbp),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1174>
lea    0x4(%rbp),%eax
and    $0x3c,%eax
mov    0x18(%rsp),%rdx
lea    0x8(%rdx),%r10
imul   %rax,%r10
mov    %rcx,0x20(%rsp)
mov    %r14d,%eax
sub    %r15b,%al
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x72f>
movzbl %al,%edi
dec    %rdi
mov    0x18(%rsp),%rsi
lea    (%rsi,%r12,1),%rcx
add    $0x7,%rcx
xor    $0x7,%rsi
xor    %eax,%eax
mov    %edi,%edx
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
cmp    %esi,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29e5>
mov    %rax,%r8
shl    $0x8,%r8
movzbl 0x1(%rcx,%rdi,1),%eax
or     %r8,%rax
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x710>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x731>
xor    %eax,%eax
mov    %r10,0x10(%rsp)
mov    %r14,0x48(%rsp)
mov    %r12,0x30(%rsp)
movabs $0x1ffffffffffffffc,%rdx
and    %rbp,%rdx
add    $0x4,%rdx
mov    %r13d,%ecx
shl    %cl,%rax
cmp    $0x3,%r9
pxor   %xmm0,%xmm0
movdqa %xmm0,0x100(%rsp)
movdqa %xmm0,0xf0(%rsp)
movdqa %xmm0,0xe0(%rsp)
movdqa %xmm0,0xd0(%rsp)
movdqa %xmm0,0xc0(%rsp)
movdqa %xmm0,0xb0(%rsp)
movdqa %xmm0,0xa0(%rsp)
movdqa %xmm0,0x90(%rsp)
cmovb  %r9,%rdx
test   %r9,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x986>
lea    0x0(,%rdx,8),%rcx
lea    0x0(,%rdx,4),%rsi
mov    %rsi,0x40(%rsp)
lea    0x2(%rcx),%rsi
mov    %rsi,0x60(%rsp)
lea    0x4(%rcx),%rsi
mov    %rsi,0x70(%rsp)
lea    0x2(,%rdx,4),%rsi
mov    %rsi,0x88(%rsp)
add    %rdx,%rdx
mov    %rcx,0x38(%rsp)
add    $0x6,%rcx
mov    %rcx,0x80(%rsp)
neg    %rbx
xor    %r13d,%r13d
lea    0x1(%rbp),%rdi
xor    %r15d,%r15d
mov    0x38(%rsp),%rcx
lea    (%rcx,%r13,1),%r8
mov    0x40(%rsp),%rcx
lea    (%rcx,%r13,1),%r11
lea    0x0(,%r13,2),%r10
add    %r13,%r10
lea    (%rdx,%r13,1),%r9
lea    0x0(,%r13,4),%rsi
add    %r13,%rsi
mov    0x80(%rsp),%rcx
lea    (%rcx,%r13,8),%rcx
sub    %r13,%rcx
inc    %r13
mov    0x60(%rsp),%r14
add    %r10,%r14
add    0x70(%rsp),%rsi
add    0x88(%rsp),%r10
neg    %rdi
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
lea    (%rbx,%r13,1),%r12
cmp    $0x1,%r12
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x8a8>
inc    %r8
inc    %r11
add    $0x3,%r14
inc    %r9
add    $0x5,%rsi
add    $0x3,%r10
add    $0x7,%rcx
lea    (%rdi,%r13,1),%r12
inc    %r12
inc    %r13
cmp    $0x1,%r12
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x870>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x986>
mov    0x18(%rsp),%rdi
dec    %edi
lea    -0x1f1aa2(%rip),%r12        # ADDR <anon.ADDR.24.llvm.ADDR+0xc11>
movslq (%r12,%rdi,4),%rdi
add    %r12,%rdi
jmp    *%rdi
mov    (%rsp),%rsi
movzbl (%rsi,%r8,1),%ecx
mov    %rsi,%r8
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x946>
mov    (%rsp),%r8
mov    -0x4(%r8,%rsi,1),%edi
movzbl (%r8,%rsi,1),%ecx
shl    $0x20,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x943>
mov    (%rsp),%r8
movzwl -0x2(%r8,%r14,1),%esi
movzbl (%r8,%r14,1),%ecx
shl    $0x10,%ecx
or     %rsi,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x946>
mov    (%rsp),%r8
mov    (%r8,%r9,4),%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x946>
mov    (%rsp),%r8
movzwl (%r8,%r11,2),%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x946>
mov    (%rsp),%r8
mov    -0x4(%r8,%r10,2),%esi
movzwl (%r8,%r10,2),%ecx
shl    $0x20,%rcx
or     %rsi,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x946>
mov    (%rsp),%r8
mov    -0x6(%r8,%rcx,1),%esi
movzwl -0x2(%r8,%rcx,1),%edi
movzbl (%r8,%rcx,1),%ecx
shl    $0x20,%rdi
or     %rsi,%rdi
shl    $0x30,%rcx
or     %rdi,%rcx
lea    0x1(%rbp),%r9
cmp    $0x8,%r15
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a63>
mov    -0x8(%r8,%r13,8),%rsi
or     %rax,%rcx
mov    %r15,%rdi
shl    $0x4,%rdi
mov    %rcx,0x90(%rsp,%rdi,1)
mov    %rsi,0x98(%rsp,%rdi,1)
inc    %r15
cmp    %r9,%r13
mov    %r9,%rdi
cmova  %r13,%rdi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x80c>
cmp    $0x9,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2843>
movzbl 0x48(%rsp),%edx
lea    0x90(%rsp),%rcx
mov    0x8(%rsp),%rdi
mov    0x30(%rsp),%rsi
mov    %rbp,%r8
call   ADDR <expanse_trie::mutate_map::write_map_immed::<false>>
mov    (%rsp),%rdx
test   %rdx,%rdx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2873>
mov    0x10(%rsp),%rsi
lea    0xf(%rsi),%rax
and    $0xfffffffffffffff0,%rax
cmp    $0x177,%rsi
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xa15>
lea    -0x1e3e20(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rsi,%rcx,1),%edi
cmp    $0xff,%rdi
mov    0x8(%rsp),%rcx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xa1a>
sub    %rax,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a51>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a45>
mov    0x8(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
mov    %rdx,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a3f>
mov    %r9d,0x10(%rsp)
mov    %rdi,0x8(%rsp)
movzbl %sil,%eax
mov    %rbp,%rdi
mov    %r12,%rsi
mov    %rdx,%rcx
mov    %eax,%edx
call   ADDR <expanse_trie::leaf::locate>
test   $0x1,%al
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    %rdx,%r8
mov    0x0(%r13),%rax
mov    (%rax,%rdx,8),%r9
cmp    $0x2,%r12d
mov    0x8(%rsp),%rcx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xeb9>
mov    $0x1,%esi
sub    %r8,%rsi
mov    (%rax,%rsi,8),%rbx
movl   $0x0,0x6b(%rsp)
movl   $0x0,0x68(%rsp)
and    $0xf,%r15d
cmp    $0x8,%r15d
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x288c>
mov    %rax,%r12
imul   %r15,%rsi
mov    $0x2,%eax
sub    %r8,%rax
imul   %r15,%rax
mov    %rax,%rcx
sub    %rsi,%rcx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28a3>
cmp    $0x8,%rax
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28a3>
cmp    %r15,%rcx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28ed>
mov    %r9,%r14
add    %rbp,%rsi
lea    0x68(%rsp),%rdi
mov    %r15,%rdx
call   *0x18cdbe(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbx,0x0(%r13)
mov    0x68(%rsp),%eax
mov    0x6b(%rsp),%ecx
mov    %eax,0x0(%rbp)
mov    %ecx,0x3(%rbp)
mov    0x10(%rsp),%eax
and    $0x70,%al
mov    %al,0xf(%r13)
mov    0x8(%rsp),%rcx
addq   $0xfffffffffffffff0,0x200(%rcx)
decq   0x208(%rcx)
mov    0x80(%rcx),%rax
mov    %rax,(%r12)
mov    %r12,0x80(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe86>
cmp    $0x19,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe10>
mov    $0x18,%eax
mov    $0x18,%r13d
cmp    $0x19,%r9
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1174>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11f2>
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
imul   $0x1010101,%ecx,%esi
shr    $0x18,%esi
lea    -0x1(%rsi),%r10
mov    %r10,%rax
cmp    $0x3,%r10
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x166d>
cmp    $0x11,%r10d
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1656>
lea    0x2(%rsi),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x166d>
movzbl %cl,%eax
cmp    $0x8,%cl
mov    $0x8,%ecx
cmovb  %eax,%ecx
movzbl %dil,%eax
movd   %eax,%xmm0
punpcklbw %xmm0,%xmm0
pshuflw $0x0,%xmm0,%xmm0
pshufd $0x44,%xmm0,%xmm0
movq   (%rbx),%xmm1
mov    $0xffffffff,%eax
shl    %cl,%eax
pcmpeqb %xmm0,%xmm1
pmovmskb %xmm1,%ecx
not    %eax
and    %ecx,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
tzcnt  %eax,%r14d
cmp    $0x2,%r14d
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x100a>
lea    0x18340a(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x448>
mov    $0x3,%esi
mov    %r14,%rdi
call   *0x18cc84(%rip)        # ADDR <_DYNAMIC+0x208>
movzbl %cl,%eax
cmp    $0x8,%cl
mov    $0x8,%ecx
cmovb  %eax,%ecx
movzbl %dil,%eax
movd   %eax,%xmm0
punpcklbw %xmm0,%xmm0
pshuflw $0x0,%xmm0,%xmm0
pshufd $0x44,%xmm0,%xmm0
movq   (%rbx),%xmm1
mov    $0xffffffff,%eax
shl    %cl,%eax
pcmpeqb %xmm0,%xmm1
pmovmskb %xmm1,%ecx
not    %eax
and    %ecx,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
tzcnt  %eax,%r14d
cmp    $0x6,%r14d
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x108a>
lea    0x183375(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x418>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0x18cc1d(%rip)        # ADDR <_DYNAMIC+0x208>
mov    %r10,%r14
mov    0x4(%rbx),%eax
xor    %edx,%edx
sub    $0x1,%rax
cmovae %rax,%rdx
cmp    $0xa0,%rdx
mov    %r15,%rdi
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe34>
mov    $0x13f,%eax
xor    %edx,%edx
pxor   %xmm0,%xmm0
mov    %rdx,%rcx
movzbl (%rbx,%rax,1),%edx
movd   %edx,%xmm1
movzbl -0x10(%rbx,%rax,1),%edx
movd   %edx,%xmm2
punpcklbw %xmm1,%xmm2
movzbl -0x20(%rbx,%rax,1),%edx
movd   %edx,%xmm1
movzbl -0x30(%rbx,%rax,1),%edx
movd   %edx,%xmm3
punpcklbw %xmm1,%xmm3
punpcklwd %xmm2,%xmm3
movzbl -0x40(%rbx,%rax,1),%edx
movd   %edx,%xmm1
movzbl -0x50(%rbx,%rax,1),%edx
movd   %edx,%xmm2
punpcklbw %xmm1,%xmm2
movzbl -0x60(%rbx,%rax,1),%edx
movd   %edx,%xmm4
movzbl -0x70(%rbx,%rax,1),%edx
movd   %edx,%xmm1
punpcklbw %xmm4,%xmm1
punpcklwd %xmm2,%xmm1
punpckldq %xmm3,%xmm1
movzbl -0x80(%rbx,%rax,1),%edx
movd   %edx,%xmm2
movzbl -0x90(%rbx,%rax,1),%edx
movd   %edx,%xmm3
punpcklbw %xmm2,%xmm3
movzbl -0xa0(%rbx,%rax,1),%edx
movd   %edx,%xmm2
movzbl -0xb0(%rbx,%rax,1),%edx
movd   %edx,%xmm4
punpcklbw %xmm2,%xmm4
punpcklwd %xmm3,%xmm4
movzbl -0xc0(%rbx,%rax,1),%edx
movd   %edx,%xmm2
movzbl -0xd0(%rbx,%rax,1),%edx
movd   %edx,%xmm3
punpcklbw %xmm2,%xmm3
movzbl -0xe0(%rbx,%rax,1),%edx
movd   %edx,%xmm2
movzbl -0xf0(%rbx,%rax,1),%edx
movd   %edx,%xmm5
punpcklbw %xmm2,%xmm5
punpcklwd %xmm3,%xmm5
punpckldq %xmm4,%xmm5
punpcklqdq %xmm1,%xmm5
pcmpeqb %xmm0,%xmm5
pmovmskb %xmm5,%edx
not    %edx
mov    %edx,%esi
shr    $1,%esi
and    $0x5555,%esi
sub    %esi,%edx
mov    %edx,%esi
and    $0x3333,%esi
shr    $0x2,%edx
and    $0x3333,%edx
add    %esi,%edx
mov    %edx,%esi
shr    $0x4,%esi
add    %edx,%esi
and    $0xf0f,%esi
mov    %esi,%edx
shr    $0x8,%edx
add    %esi,%edx
movzbl %dl,%edx
add    %rcx,%rdx
cmp    $0xa1,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe34>
add    $0x100,%rax
cmp    $0x113f,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xc9e>
mov    %edx,0x4(%rbx)
test   %rdx,%rdx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe37>
lock subq $0x1040,0x200(%rdi)
lock decq 0x208(%rdi)
mov    %rbx,%rdi
call   *0x18cab4(%rip)        # ADDR <free@GLIBC_2.2.5>
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe86>
cmp    $0x21,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x115c>
mov    $0x20,%eax
mov    $0x20,%r13d
cmp    $0x21,%r9
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1174>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11f2>
mov    %edx,0x4(%rbx)
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe68>
mov    0x8(%r12),%rax
movzbl %r13b,%ecx
lea    -0x1e63b5(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rsi,%rcx,8),%rcx
mov    %rcx,%rsi
and    %rax,%rsi
dec    %rsi
xor    %rax,%rsi
and    %rcx,%rsi
xor    %rax,%rsi
mov    %rsi,0x8(%r12)
mov    $0x1,%eax
cmp    $0xa0,%rdx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1104>
movzbl %r13b,%edx
mov    %r12,%rsi
call   ADDR <expanse_trie::mutate::downgrade_u_to_b::<false>>
mov    $0x1,%eax
mov    %r14,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
lock sub %rax,0x200(%rdx)
lock decq 0x208(%rdx)
mov    %rsi,%rdi
call   *0x18ca0c(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    $0x1,%eax
mov    %rbx,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    %rax,0x18(%rsp)
and    $0xf,%r15d
lea    -0x1(%r12),%rsi
lea    0x2(%r12),%eax
and    $0xfffffffc,%eax
cmp    $0x3,%bl
mov    %rsi,0x38(%rsp)
cmovb  %rsi,%rax
lea    0x3(%r12),%esi
and    $0xfffffffc,%esi
cmp    $0x2,%bl
cmovb  %r12,%rsi
cmp    %rsi,%rax
mov    %r8,(%rsp)
mov    %r9,0x20(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1592>
mov    0x0(%rbp),%eax
mov    0x3(%rbp),%ecx
mov    %ecx,0x53(%rsp)
mov    %eax,0x50(%rsp)
mov    %r12,%rsi
imul   %r15,%rsi
cmp    $0x8,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2910>
lea    0x1(%r8),%rbx
mov    %rbx,%rdi
imul   %r15,%rdi
mov    %rsi,%rdx
sub    %rdi,%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x293b>
imul   %r15,%r8
mov    %rdx,%rcx
xor    $0x7,%rcx
cmp    %rcx,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2965>
lea    (%rsp,%rdi,1),%rsi
add    $0x50,%rsi
lea    (%rsp,%r8,1),%rdi
add    $0x50,%rdi
call   *0x18ca14(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    -0x1(%r12),%r14
imul   %r14,%r15
cmp    $0x7,%r15
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2997>
lea    (%rsp,%r15,1),%rdi
add    $0x50,%rdi
xor    $0x7,%r15
xor    %esi,%esi
mov    %r15,%rdx
call   *0x18ca0a(%rip)        # ADDR <memset@GLIBC_2.2.5>
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28bb>
cmp    %r12,%rbx
mov    0x38(%rsp),%r15
mov    (%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfba>
mov    0x18(%rsp),%rax
lea    (%rax,%rcx,8),%rdi
lea    (%rax,%rbx,8),%rsi
mov    %r15,%rdx
sub    %rcx,%rdx
shl    $0x3,%rdx
call   *0x18c9b6(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x50(%rsp),%eax
mov    0x53(%rsp),%ecx
mov    %ecx,0x3(%rbp)
mov    %eax,0x0(%rbp)
mov    0x10(%rsp),%ecx
and    $0x70,%cl
lea    -0x1(%r15),%eax
or     %cl,%al
mov    %al,0xf(%r13)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a45>
cmp    $0x2,%cl
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfef>
mov    $0x1,%r14d
cmp    %dil,0x9(%rbp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x100a>
xor    %eax,%eax
cmp    $0x3,%cl
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    $0x2,%r14d
cmp    %dil,0xa(%rbp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    %r9d,0x10(%rsp)
mov    %r14d,%eax
shl    $0x4,%eax
lea    (%rax,%rbp,1),%r15
add    $0x10,%r15
movzbl %sil,%ecx
mov    %r8,%rdi
mov    %r8,0x8(%rsp)
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate_map::map_remove::<false, false>>
cmp    $0x1,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    %rdx,%r10
cmpb   $0x0,0xf(%r15)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x10c9>
mov    %r10,%r15
lea    0x10(%rbp),%rsi
movzbl 0x4(%rbp),%ecx
mov    $0x3,%edx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1499>
cmp    $0x2,%cl
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x106f>
mov    $0x1,%r14d
cmp    %dil,0x9(%rbp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x108a>
xor    %eax,%eax
cmp    $0x3,%cl
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    $0x2,%r14d
cmp    %dil,0xa(%rbp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    %r9d,0x10(%rsp)
mov    %r14d,%eax
shl    $0x4,%eax
lea    (%rax,%rbp,1),%r15
add    $0x10,%r15
movzbl %sil,%ecx
mov    %r8,%rdi
mov    %r8,0x8(%rsp)
mov    %r15,%rsi
call   ADDR <expanse_trie::mutate_map::map_remove::<false, false>>
cmp    $0x1,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    %rdx,%r10
cmpb   $0x0,0xf(%r15)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1489>
mov    $0x1,%eax
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    0x8(%r12),%rcx
lea    -0x1e664c(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r13,8),%rdx
mov    %rdx,%rsi
and    %rcx,%rsi
dec    %rsi
xor    %rcx,%rsi
and    %rdx,%rsi
xor    %rcx,%rsi
mov    %rsi,0x8(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    %r14,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
movzwl 0x60(%rbp,%rbx,2),%r13d
cmp    $0x1,%r13
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x17b5>
movq   $0x0,0x20(%rbp,%rbx,8)
mov    0x8(%rsp),%r8
addq   $0xfffffffffffffff0,0x200(%r8)
decq   0x208(%r8)
mov    0x80(%r8),%rax
mov    %rax,(%rsi)
mov    %rsi,0x80(%r8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x26de>
lea    0x3(%rbp),%r13d
and    $0x3c,%r13d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11f2>
lea    0x3(%rbp),%r13
and    $0xfffffffffffffffc,%r13
mov    %rbp,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
cmp    %rax,%r13
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11f2>
mov    %rcx,0x20(%rsp)
lea    (%rsi,%rbx,8),%rdi
lea    (%rsi,%rax,8),%r15
lea    0x1(%rbx),%r14
lea    (%rsi,%rbx,8),%rsi
add    $0x8,%rsi
mov    %r12,0x30(%rsp)
mov    %rbp,%r12
sub    %rbx,%r12
lea    0x0(,%r12,8),%rdx
mov    0x18c7c9(%rip),%r13        # ADDR <memmove@GLIBC_2.2.5>
call   *%r13
mov    0x18(%rsp),%rax
imul   %rax,%r14
add    %r15,%r14
imul   %rax,%rbx
add    %r15,%rbx
imul   %rax,%r12
mov    %rbx,%rdi
mov    %r14,%rsi
mov    %r12,%rdx
call   *%r13
mov    0x20(%rsp),%r10
dec    %rbp
mov    0x30(%rsp),%rcx
mov    0x8(%rcx),%rax
xor    %rax,%rbp
and    0x38(%rsp),%rbp
xor    %rax,%rbp
mov    %rbp,0x8(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x164c>
mov    0x18(%rsp),%rax
lea    0x8(%rax),%rsi
imul   %rsi,%r13
lea    0xf(%r13),%r15
and    $0xfffffffffffffff0,%r15
cmp    $0x177,%r13
mov    %rcx,0x20(%rsp)
mov    %rsi,0x48(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1271>
lea    -0x1e4668(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl 0x0(%r13,%rax,1),%r14d
cmp    $0xff,%r14
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x12bb>
add    %r15,0x200(%rdx)
incq   0x208(%rdx)
incq   0x218(%rdx)
cmp    $0x3e,%r14b
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a7a>
mov    0x10(%rdx,%r14,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1319>
mov    (%rdi),%rax
mov    %rax,0x10(%rdx,%r14,8)
mov    %rdi,%r15
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x13a8>
lock add %r15,0x200(%rdx)
lock incq 0x208(%rdx)
lock incq 0x218(%rdx)
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r13
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a07>
mov    $0x1,%esi
mov    %r13,%rdi
call   *0x18c6de(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r15
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x13b6>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x253a>
lock add %r15,0x200(%rdx)
lock incq 0x208(%rdx)
lock incq 0x218(%rdx)
cmp    $0xf,%r13
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x129c>
movq   $0x0,0x90(%rsp)
lea    0x90(%rsp),%rdi
mov    $0x10,%esi
mov    %r13,%rdx
call   *0x18c685(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x253a>
mov    0x90(%rsp),%r15
test   %r15,%r15
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x13a8>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x253a>
cmp    $0x100,%r13
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x129c>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2900>
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rcx
mov    %rcx,(%rax)
mov    %r14,0x8(%rax)
mov    %rax,0x220(%rdi)
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a8f>
mov    %rax,%r8
cmp    $0x7e0,%r15
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x13a1>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r15w
movzwl %ax,%eax
dec    %rax
mov    %rax,%rcx
imul   %r15,%rcx
lea    (%r8,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x10(%rdi,%r14,8),%rsi
mov    %rsi,0x40(%r8,%rcx,1)
mov    %rdx,0x10(%rdi,%r14,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x137a>
add    $0x40,%r8
mov    %r8,%r15
mov    %r15,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0x18c5da(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r12,0x30(%rsp)
lea    0x0(,%rbx,8),%rdx
mov    0x18c4d6(%rip),%r13        # ADDR <memcpy@GLIBC_2.14>
mov    %r15,%rdi
mov    (%rsp),%r14
mov    %r14,%rsi
call   *%r13
lea    0x1(%rbx),%rax
mov    %rax,0x40(%rsp)
lea    (%r14,%rbx,8),%rsi
add    $0x8,%rsi
mov    %r15,%r12
lea    (%r15,%rbx,8),%rdi
mov    %rbp,%rax
sub    %rbx,%rax
mov    %rax,0x60(%rsp)
lea    0x0(,%rax,8),%rdx
call   *%r13
mov    $0x1,%ecx
mov    $0x2,%eax
lea    0x1(%rbp),%rdx
cmp    $0x3,%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18d9>
cmp    $0x11,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1438>
cmp    $0x3,%rdx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1453>
mov    $0x2,%ecx
mov    $0x4,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18d9>
cmp    $0x19,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1464>
mov    $0x18,%eax
mov    $0x18,%ecx
cmp    $0x11,%rdx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1459>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18d9>
lea    0x4(%rbp),%eax
and    $0x3c,%eax
lea    0x3(%rbp),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18d9>
cmp    $0x21,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16fa>
xor    %eax,%eax
cmp    $0x19,%rdx
setne  %al
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18d9>
mov    %r10,%r15
lea    0x10(%rbp),%rsi
movzbl 0x4(%rbp),%ecx
mov    $0x7,%edx
mov    %rbx,%rdi
mov    %r14,%r8
call   ADDR <expanse_trie::mutate::linear_remove_slot>
movzbl 0x4(%rbp),%eax
mov    %eax,%edx
dec    %dl
mov    %dl,0x4(%rbp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x170f>
add    $0xf6,%al
cmp    $0xf8,%al
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a9c>
movzbl (%rbx),%ecx
and    $0xf,%cl
mov    $0x1,%eax
shl    %cl,%eax
cmp    $0x1,%dl
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%r8d
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x171b>
movzbl 0x9(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x2,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x171b>
movzbl 0xa(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x3,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x171b>
movzbl 0xb(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x4,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x171b>
movzbl 0xc(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x5,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x171b>
movzbl 0xd(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x6,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x171b>
movzbl 0xe(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x7,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1711>
movzbl 0xf(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1711>
mov    0x8(%r13),%ecx
movzbl 0xc(%r13),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1610>
movzwl 0x8(%r13),%ecx
movzbl 0xa(%r13),%eax
shl    $0x10,%eax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1635>
mov    0x0(%rbp),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1635>
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1635>
lea    0x0(,%rax,8),%rdx
lea    0x8(%rdx),%ebx
and    $0x1ff0,%ebx
lea    -0x1e49f1(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rdi,%rax,8),%r14d
cmp    $0xff,%r14
mov    %rbp,0x30(%rsp)
mov    %rsi,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2193>
add    %rbx,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3e,%r14b
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a7a>
mov    0x10(%rcx,%r14,8),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x21d5>
mov    0x0(%rbp),%rax
mov    %rax,0x10(%rcx,%r14,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2255>
movzwl 0x0(%rbp),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1635>
mov    0x8(%r13),%ecx
movzwl 0xc(%r13),%eax
shl    $0x20,%rax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1635>
mov    0x8(%r13),%ecx
movzwl 0xc(%r13),%esi
movzbl 0xe(%r13),%eax
shl    $0x20,%rsi
or     %rcx,%rsi
shl    $0x30,%rax
or     %rsi,%rax
cmp    %rdx,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    0x0(%r13),%r10
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%r13)
mov    $0x1,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    $0x18,%eax
cmp    $0x19,%r10d
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x166d>
xor    %eax,%eax
cmp    $0x21,%r10d
setb   %al
shl    $0x5,%eax
mov    %rsi,%rcx
cmp    $0x3,%esi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1692>
cmp    $0x11,%esi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1682>
lea    0x3(%rsi),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1692>
xor    %ecx,%ecx
cmp    $0x19,%esi
setae  %cl
lea    0x18(,%rcx,8),%rcx
lea    (%r8,%r9,8),%r11
cmp    %ecx,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16c8>
lea    0x8(%r11),%rsi
sub    %r9,%r10
shl    $0x3,%r10
mov    %r14,%r15
mov    %rdi,%r14
mov    %r11,%rdi
mov    %rdx,%rbx
mov    %r10,%rdx
call   *0x18c2b6(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r14,%rdi
mov    %r15,%r14
mov    %rbx,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1be1>
mov    %r10,%rcx
cmp    $0x3,%r10d
mov    %r14,0x48(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16e6>
cmp    $0x11,%r10d
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x17f5>
lea    0x2(%rsi),%ecx
and    $0x3c,%ecx
shl    $0x3,%rcx
lea    0x8(%rcx),%r14d
and    $0x3f0,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1812>
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18c6>
mov    $0x20,%ecx
mov    $0x24,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18d9>
xor    %eax,%eax
mov    0x8(%rsp),%rdi
mov    0x10(%rsp),%r8d
mov    %ax,0x6(%rbp)
mov    (%r12),%rax
movzbl 0x4(%rax),%ecx
test   %cl,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1776>
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1758>
mov    0x8(%r12),%rax
lea    -0x1e6ca5(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r13,8),%rdx
mov    %rdx,%rsi
and    %rax,%rsi
dec    %rsi
xor    %rax,%rsi
and    %rdx,%rsi
xor    %rax,%rsi
mov    %rsi,0x8(%r12)
cmp    $0x1,%r8b
sete   %al
cmp    $0x3,%cl
setae  %cl
or     %al,%cl
mov    $0x1,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x17e0>
mov    %r15,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
xor    %ecx,%ecx
xor    %edx,%edx
cmp    $0x1,%r8b
setne  %cl
sete   %dl
shl    $0x6,%edx
or     $0xffffffffffffff80,%rdx
add    %rdx,0x200(%rdi)
decq   0x208(%rdi)
shl    $0x4,%ecx
mov    0x18(%rcx,%rdi,1),%rdx
mov    %rdx,(%rax)
mov    %rax,0x18(%rcx,%rdi,1)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x17e8>
lea    -0x1(%r13),%rdx
mov    %rdx,%rax
cmp    $0x3,%rdx
mov    0x8(%rsp),%r8
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23f9>
cmp    $0x11,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23de>
lea    0x2(%r13),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23f9>
mov    %r12,%rsi
call   ADDR <expanse_trie::mutate::downgrade_l7_to_l3::<false>>
mov    $0x1,%eax
mov    %r15,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
xor    %eax,%eax
cmp    $0x21,%r10
setb   %al
shl    $0x8,%eax
cmp    $0x19,%r10
mov    $0xc0,%r14d
cmovae %rax,%r14
mov    %r14,%rcx
lea    -0x1e4c60(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rax,1),%r15d
cmp    $0xff,%r15
mov    %rdx,0x40(%rsp)
mov    %rdi,0x8(%rsp)
mov    %r8,(%rsp)
mov    %rsi,0x18(%rsp)
mov    %r9,0x10(%rsp)
mov    %r10,0x38(%rsp)
mov    %r11,0x30(%rsp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1884>
add    %r14,0x200(%rdi)
incq   0x208(%rdi)
incq   0x218(%rdi)
cmp    $0x3e,%r15b
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ab3>
mov    0x10(%rdi,%r15,8),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a61>
mov    0x0(%rbp),%rax
mov    %rax,0x10(%rdi,%r15,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ae2>
lock add %r14,0x200(%rdi)
lock incq 0x208(%rdi)
lock incq 0x218(%rdi)
mov    $0x10,%esi
mov    %rcx,%rdi
mov    %rcx,%r14
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rbp
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1af0>
mov    $0x10,%edi
mov    %r14,%rsi
call   *0x18c042(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %rbp,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
lea    0x3(%rbp),%rcx
and    $0xfffffffffffffffc,%rcx
mov    (%rsp),%rdx
lea    (%rdx,%rax,8),%r15
mov    %r12,0x70(%rsp)
lea    (%r12,%rcx,8),%r12
mov    0x18(%rsp),%r13
imul   %r13,%rbx
mov    %r12,%rdi
mov    %r15,%rsi
mov    %rbx,%rdx
mov    0x18bf9d(%rip),%r14        # ADDR <memcpy@GLIBC_2.14>
call   *%r14
mov    0x40(%rsp),%rsi
imul   %r13,%rsi
add    %r15,%rsi
add    %r12,%rbx
mov    0x60(%rsp),%rdx
imul   %r13,%rdx
mov    %rbx,%rdi
call   *%r14
mov    $0x2,%edi
lea    0x1(%rbp),%rsi
mov    0x30(%rsp),%rdx
mov    0x10(%rsp),%eax
mov    %al,0xf(%rdx)
lea    -0x1(%rbp),%rax
mov    0x8(%rdx),%rcx
xor    %rcx,%rax
and    0x38(%rsp),%rax
xor    %rcx,%rax
mov    0x70(%rsp),%rcx
mov    %rcx,(%rdx)
mov    %rax,0x8(%rdx)
cmp    $0x3,%rsi
mov    0x8(%rsp),%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x198e>
cmp    $0x11,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1974>
add    $0x4,%ebp
and    $0x3c,%ebp
mov    %rbp,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x198e>
mov    $0x18,%edi
cmp    $0x19,%rsi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x198e>
mov    $0x20,%edi
cmp    $0x21,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a54>
imul   0x48(%rsp),%rdi
lea    0xf(%rdi),%rax
and    $0xfffffffffffffff0,%rax
cmp    $0x177,%rdi
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19e4>
lea    -0x1e4df3(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rdi,%rcx,1),%edi
cmp    $0xff,%rdi
mov    (%rsp),%rcx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a2c>
sub    %rax,0x200(%rdx)
decq   0x208(%rdx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a51>
mov    0x10(%rdx,%rdi,8),%rax
mov    %rax,(%rcx)
mov    %rcx,0x10(%rdx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a45>
lock sub %rax,0x200(%rdx)
lock decq 0x208(%rdx)
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%rdi
mov    (%rsp),%rcx
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a3c>
lea    -0x1e7009(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0x18924b(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x1823d4(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x2f(%rsp),%rdx
mov    $0x11,%esi
call   *0x18bf24(%rip)        # ADDR <_DYNAMIC+0x2c8>
lock sub %rax,0x200(%rdx)
lock decq 0x208(%rdx)
mov    %rcx,%rdi
call   *0x18be73(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    $0x1,%eax
mov    0x20(%rsp),%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
and    $0xfffffffffffffffc,%rbp
add    $0x4,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x196f>
mov    %rcx,0x60(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2900>
mov    %rax,%rbp
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,0x0(%rbp)
mov    %r15,0x8(%rbp)
mov    %rbp,0x220(%rdi)
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a8f>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r14w
movzwl %ax,%eax
dec    %rax
mov    %rax,%rcx
imul   %r14,%rcx
lea    (%rcx,%rbp,1),%rdx
add    $0x40,%rdx
mov    0x10(%rdi,%r15,8),%rsi
mov    %rsi,0x40(%rbp,%rcx,1)
mov    %rdx,0x10(%rdi,%r15,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ab2>
add    $0x40,%rbp
mov    0x60(%rsp),%rcx
mov    %rbp,%rdi
xor    %esi,%esi
mov    %rcx,%rdx
call   *0x18bea0(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x10(%rsp),%r14
lea    0x0(,%r14,8),%r15d
mov    %rbp,%rdi
mov    (%rsp),%rsi
mov    %r15,%rdx
call   *0x18bd93(%rip)        # ADDR <memcpy@GLIBC_2.14>
add    %rbp,%r15
mov    0x30(%rsp),%rsi
add    $0x8,%rsi
mov    0x38(%rsp),%rdx
sub    %r14,%rdx
shl    $0x3,%rdx
mov    %r15,%rdi
call   *0x18bd72(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,0x20(%r13,%rbx,8)
mov    0x18(%rsp),%rdx
cmp    $0x3,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b49>
mov    0x8(%rsp),%rcx
mov    0x48(%rsp),%r14
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b70>
cmp    $0x11,%edx
mov    0x8(%rsp),%rcx
mov    0x48(%rsp),%r14
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b60>
add    $0x3,%edx
and    $0x3c,%edx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b70>
xor    %eax,%eax
cmp    $0x19,%edx
setae  %al
lea    0x18(,%rax,8),%rdx
mov    (%rsp),%rsi
shl    $0x3,%edx
lea    0x8(%rdx),%rax
and    $0xfffffffffffffff0,%rax
lea    -0x1e4fcd(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rdx,%rdi,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1bbd>
sub    %rax,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3d,%dil
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a51>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rsi)
mov    %rsi,0x10(%rcx,%rdi,8)
mov    %rcx,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1bdc>
mov    %rcx,%rbx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
mov    %rsi,%rdi
call   *0x18bcdf(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
mov    0x40(%rsp),%rdx
mov    %edx,%eax
shr    $0x3,%eax
mov    $0xfffffffffffffffe,%rsi
mov    %edx,%ecx
rol    %cl,%rsi
and    $0x18,%eax
and    %rsi,0x0(%r13,%rax,1)
mov    0x8(%r12),%rax
movzbl %al,%ecx
cmp    $0x15,%rcx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c43>
movzwl 0xd(%r12),%eax
mov    %ax,0x7c(%rsp)
mov    0x9(%r12),%eax
mov    %eax,0x78(%rsp)
cmp    $0x2,%r14b
mov    %rdi,0x8(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c59>
movzbl %r14b,%edx
mov    %r12,%rdi
mov    $0x1,%esi
call   *0x18bfd7(%rip)        # ADDR <_DYNAMIC+0x588>
shl    $0x8,%rax
mov    %rax,(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c61>
dec    %rcx
and    $0xffffffffffffff00,%rax
or     %rcx,%rax
mov    %rax,0x8(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a45>
movq   $0x0,(%rsp)
movq   $0x0,0x290(%rsp)
mov    0x0(%r13),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c7a>
xor    %ecx,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c9f>
mov    0x8(%r13),%rax
mov    $0x40,%cl
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c9f>
mov    0x10(%r13),%rax
mov    $0x80,%cl
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c9f>
mov    0x18(%r13),%rax
mov    $0xc0,%cl
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1dcd>
tzcnt  %rax,%rax
or     %cl,%al
lea    0x98(%rsp),%rsi
xor    %edx,%edx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ce1>
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
shl    $0x6,%dil
tzcnt  %r9,%rax
or     %dil,%al
add    $0x10,%rsi
lea    0x1(%rdx),%rdi
cmp    $0x1f,%rdx
mov    %rdi,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a04>
movzbl %al,%ecx
mov    %ecx,%edi
shr    $0x5,%dil
mov    $0xffffffff,%r8d
shl    %cl,%r8d
not    %r8d
movzbl %dil,%edi
and    0x0(%r13,%rdi,4),%r8d
mov    %r8d,%r9d
shr    $1,%r9d
and    $0x15555555,%r9d
sub    %r9d,%r8d
mov    %r8d,%r9d
and    $0x33333333,%r9d
shr    $0x2,%r8d
and    $0x33333333,%r8d
add    %r9d,%r8d
mov    %r8d,%r9d
shr    $0x4,%r9d
add    %r8d,%r9d
and    $0xf0f0f0f,%r9d
imul   $0x1010101,%r9d,%r8d
mov    0x20(%r13,%rdi,8),%rdi
shr    $0x18,%r8d
mov    (%rdi,%r8,8),%rdi
mov    %rcx,-0x8(%rsi)
mov    %rdi,(%rsi)
cmp    $0xff,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1dc2>
inc    %al
mov    %eax,%edi
shr    $0x6,%dil
movzbl %dil,%r8d
mov    0x0(%r13,%r8,8),%r9
mov    %eax,%ecx
shr    %cl,%r9
shl    %cl,%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1cc0>
lea    0x1(%r8),%rdi
cmp    $0x4,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1dc2>
mov    0x8(%r13,%r8,8),%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1cc0>
lea    0x2(%r8),%rdi
cmp    $0x4,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1dc2>
mov    0x10(%r13,%r8,8),%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1cc0>
lea    0x3(%r8),%rdi
cmp    $0x4,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1dc2>
mov    0x18(%r13,%r8,8),%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1cc0>
inc    %rdx
mov    %rdx,0x290(%rsp)
lea    0x9(%r12),%rax
mov    %rax,0x18(%rsp)
movabs $0x7fffffff0,%rbx
lea    0x2a0(%rsp),%rdi
lea    0x90(%rsp),%rsi
mov    $0x208,%edx
call   *0x18baa4(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    $0x4,%ebp
lea    -0x1e524f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
mov    0x8(%rsp),%rsi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1e5a>
lock sub %rcx,0x200(%rsi)
lock decq 0x208(%rsi)
mov    %r14,%r15
mov    %rsi,%r14
mov    %r13,%rbx
mov    %r12,%r13
mov    %r8,%r12
call   *0x18ba84(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %r12,%r8
mov    %r13,%r12
mov    %rbx,%r13
mov    %r14,%rsi
mov    %r15,%r14
movabs $0x7fffffff0,%rbx
inc    %rbp
cmp    $0xc,%rbp
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1f10>
mov    -0x10(%r13,%rbp,4),%eax
test   %eax,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1e4d>
mov    0x0(%r13,%rbp,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x282a>
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
cmp    $0x3,%eax
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ec7>
cmp    $0x11,%eax
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1eb7>
add    $0x3,%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ec7>
xor    %ecx,%ecx
cmp    $0x19,%eax
setae  %cl
lea    0x18(,%rcx,8),%rax
lea    0x0(,%rax,8),%edx
lea    0x8(,%rax,8),%ecx
and    %ebx,%ecx
movzbl (%rdx,%r8,1),%eax
cmp    $0xff,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1e0f>
sub    %rcx,0x200(%rsi)
decq   0x208(%rsi)
cmp    $0x3d,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a16>
mov    0x10(%rsi,%rax,8),%rcx
mov    %rcx,(%rdi)
mov    %rdi,0x10(%rsi,%rax,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1e4d>
addq   $0xffffffffffffff80,0x200(%rsi)
decq   0x208(%rsi)
mov    0x28(%rsi),%rax
mov    %rax,0x0(%r13)
mov    %r13,0x28(%rsi)
test   %r14b,%r14b
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a2b>
mov    0x4a0(%rsp),%r9
mov    $0x7,%al
movzbl %al,%eax
div    %r14b
movzbl %al,%eax
cmp    %rax,%r9
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2129>
movq   $0x0,0x290(%rsp)
test   %r9,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2174>
mov    0x2a0(%rsp),%rax
or     (%rsp),%rax
mov    0x2a8(%rsp),%rcx
mov    %rax,0x90(%rsp)
mov    %rcx,0x98(%rsp)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
cmp    $0x1,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2177>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ac8>
mov    0x2b0(%rsp),%rax
or     (%rsp),%rax
mov    0x2b8(%rsp),%rcx
shl    $0x4,%r8
mov    %rax,0x90(%rsp,%r8,1)
mov    %rcx,0x98(%rsp,%r8,1)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
cmp    $0x2,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2177>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ac8>
mov    0x2c0(%rsp),%rax
or     (%rsp),%rax
mov    0x2c8(%rsp),%rcx
shl    $0x4,%r8
mov    %rax,0x90(%rsp,%r8,1)
mov    %rcx,0x98(%rsp,%r8,1)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
cmp    $0x3,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2177>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ac8>
mov    0x2d0(%rsp),%rax
or     (%rsp),%rax
mov    0x2d8(%rsp),%rcx
shl    $0x4,%r8
mov    %rax,0x90(%rsp,%r8,1)
mov    %rcx,0x98(%rsp,%r8,1)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
cmp    $0x4,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2177>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ac8>
mov    0x2e0(%rsp),%rax
or     (%rsp),%rax
mov    0x2e8(%rsp),%rcx
shl    $0x4,%r8
mov    %rax,0x90(%rsp,%r8,1)
mov    %rcx,0x98(%rsp,%r8,1)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
cmp    $0x5,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2177>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ac8>
mov    (%rsp),%rcx
or     0x2f0(%rsp),%rcx
mov    0x2f8(%rsp),%rax
shl    $0x4,%r8
mov    %rcx,0x90(%rsp,%r8,1)
mov    %rax,0x98(%rsp,%r8,1)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2177>
lea    0x2a0(%rsp),%rcx
mov    %rsi,%rdi
mov    %r12,%rsi
mov    $0x1,%edx
mov    %r9,%r8
call   ADDR <expanse_trie::mutate_map::build_map_leaf::<false>>
cmp    $0x2,%r14b
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a45>
movzbl %r14b,%edx
cmp    $0x8,%r14b
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29cb>
dec    %rdx
lea    0x78(%rsp),%rsi
mov    0x18(%rsp),%rdi
call   *0x18b731(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a45>
xor    %r8d,%r8d
movzbl %r14b,%edx
lea    0x90(%rsp),%rcx
mov    %rsi,%rdi
mov    %r12,%rsi
call   ADDR <expanse_trie::mutate_map::write_map_immed::<false>>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a45>
lock add %rbx,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
mov    $0x10,%esi
mov    %rdx,%rdi
mov    %rdx,%rbx
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rbp
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2260>
mov    $0x10,%edi
mov    %rbx,%rsi
call   *0x18b733(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %rdx,0x40(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2900>
mov    %rax,%rbp
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,0x0(%rbp)
mov    %r14,0x8(%rbp)
mov    %rbp,0x220(%rdi)
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a8f>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %bx
movzwl %ax,%eax
mov    0x40(%rsp),%rdx
dec    %rax
mov    %rax,%rcx
imul   %rbx,%rcx
lea    (%rcx,%rbp,1),%r8
add    $0x40,%r8
mov    0x10(%rdi,%r14,8),%rsi
mov    %rsi,0x40(%rbp,%rcx,1)
mov    %r8,0x10(%rdi,%r14,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x222a>
add    $0x40,%rbp
mov    %rbp,%rdi
xor    %esi,%esi
call   *0x18b730(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    (%rsp),%r8
test   %r8,%r8
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2283>
lea    0x0(,%r8,8),%rdx
mov    %rbp,%rdi
mov    0x18(%rsp),%rsi
call   *0x18b621(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    (%rsp),%r8
lea    0x1(%r8),%r14
cmp    %r12,%r14
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x22b6>
mov    0x18(%rsp),%rax
lea    (%rax,%r14,8),%rsi
lea    0x0(,%r8,8),%rdi
add    %rbp,%rdi
lea    -0x1(%r12),%rdx
sub    %r8,%rdx
shl    $0x3,%rdx
call   *0x18b5ee(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    (%rsp),%r8
lea    0x8(%r13),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x5b(%rsp)
mov    %eax,0x58(%rsp)
imul   %r15,%r12
cmp    $0x8,%r12
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2924>
imul   %r15,%r14
mov    %r12,%rdx
sub    %r14,%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x294d>
imul   %r15,%r8
mov    %rdx,%rax
xor    $0x7,%rax
cmp    %rax,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x297e>
lea    (%rsp,%r14,1),%rsi
add    $0x58,%rsi
lea    (%rsp,%r8,1),%rdi
add    $0x58,%rdi
call   *0x18b661(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x38(%rsp),%r14
imul   %r14,%r15
cmp    $0x7,%r15
mov    0x18(%rsp),%rbx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29b1>
lea    (%rsp,%r15,1),%rdi
add    $0x58,%rdi
xor    $0x7,%r15
xor    %esi,%esi
mov    %r15,%rdx
call   *0x18b652(%rip)        # ADDR <memset@GLIBC_2.2.5>
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28d4>
mov    %rbp,0x0(%r13)
mov    0x58(%rsp),%eax
mov    0x5b(%rsp),%ecx
mov    0x30(%rsp),%rdx
mov    %ecx,0x3(%rdx)
mov    %eax,(%rdx)
mov    0x10(%rsp),%eax
and    $0x70,%al
dec    %r14b
or     %al,%r14b
mov    %r14b,0xf(%r13)
mov    0x48(%rsp),%rax
lea    0x0(,%rax,8),%ecx
lea    0x8(,%rax,8),%eax
and    $0xfffffff0,%eax
lea    -0x1e57d1(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rdx,1),%edi
cmp    $0xff,%rdi
mov    0x8(%rsp),%rcx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23c6>
sub    %rax,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a51>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a45>
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
mov    %rbx,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a3f>
mov    $0x18,%eax
cmp    $0x19,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23f9>
mov    $0x20,%eax
cmp    $0x21,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23f9>
lea    0x2(%r13),%eax
and    $0xfffffffc,%eax
mov    %r13,%rcx
cmp    $0x3,%r13w
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2432>
cmp    $0x11,%r13w
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2413>
lea    0x3(%r13),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2432>
mov    $0x18,%ecx
cmp    $0x19,%r13w
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2432>
mov    $0x20,%ecx
cmp    $0x21,%r13w
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2432>
lea    0x3(%r13),%ecx
and    $0xfffffffc,%ecx
add    %rsi,%rdi
cmp    %ecx,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x245a>
lea    0x10(%rdi),%rsi
sub    %r15,%rdx
shl    $0x4,%rdx
mov    %r10,%r15
call   *0x18b523(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r15,%r10
mov    0x8(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x26de>
mov    %rdx,%rcx
cmp    $0x3,%edx
mov    %r10,0x20(%rsp)
mov    %r13,0x10(%rsp)
mov    %rsi,(%rsp)
mov    %rdi,0x38(%rsp)
mov    %rdx,0x48(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2486>
cmp    $0x11,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24d8>
lea    0x2(%r13),%ecx
and    $0x3c,%ecx
mov    %rcx,%r9
shl    $0x4,%r9
lea    -0x1e58db(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%r9,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2506>
add    %r9,0x200(%r8)
incq   0x208(%r8)
incq   0x218(%r8)
cmp    $0x3e,%r13b
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2add>
mov    0x10(%r8,%r13,8),%r11
test   %r11,%r11
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2548>
mov    (%r11),%rax
mov    %rax,0x10(%r8,%r13,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25d2>
mov    $0x180,%r9d
cmp    $0x19,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2506>
mov    $0x200,%r9d
cmp    $0x21,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2506>
mov    0x10(%rsp),%rax
lea    0x2(%rax),%ecx
and    $0xfffffffc,%ecx
mov    %ecx,%r9d
shl    $0x4,%r9d
cmp    $0x15,%ax
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x248d>
lock add %r9,0x200(%r8)
lock incq 0x208(%r8)
lock incq 0x218(%r8)
mov    $0x10,%esi
mov    %r9,%rdi
mov    %r9,%r13
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rdi
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25e6>
mov    $0x10,%edi
mov    %r13,%rsi
call   *0x18b3c0(%rip)        # ADDR <_DYNAMIC+0x280>
cmp    $0x10,%ecx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x251e>
mov    %rcx,0x40(%rsp)
mov    %r9,0x18(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2900>
mov    %rax,%r11
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%r11)
mov    %r13,0x8(%r11)
mov    %r11,0x220(%rdi)
cmpq   $0x0,0x40(%rsp)
mov    0x18(%rsp),%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a8f>
mov    $0xfc0,%eax
xor    %edx,%edx
div    %r9d
dec    %rax
mov    %rax,%rcx
imul   %r9,%rcx
lea    (%r11,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x10(%rdi,%r13,8),%rsi
mov    %rsi,0x40(%r11,%rcx,1)
mov    %rdx,0x10(%rdi,%r13,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25a7>
add    $0x40,%r11
mov    %r11,%rdi
xor    %esi,%esi
mov    %r9,%rdx
mov    %r11,%r13
call   *0x18b3ad(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r13,%rdi
mov    %rdi,0x18(%rsp)
mov    %r15d,%r13d
shl    $0x4,%r13d
mov    (%rsp),%rsi
mov    %r13,%rdx
call   *0x18b2a1(%rip)        # ADDR <memcpy@GLIBC_2.14>
add    0x18(%rsp),%r13
mov    0x38(%rsp),%rsi
add    $0x10,%rsi
mov    0x48(%rsp),%rdx
sub    %r15,%rdx
shl    $0x4,%rdx
mov    %r13,%rdi
call   *0x18b27e(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x18(%rsp),%rax
mov    %rax,0x20(%rbp,%rbx,8)
mov    0x10(%rsp),%r13
mov    %r13,%rax
cmp    $0x3,%r13w
mov    0x8(%rsp),%r8
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x264e>
cmp    $0x11,%r13w
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2696>
lea    0x3(%r13),%eax
and    $0x3c,%eax
shl    $0x4,%eax
lea    -0x1e5a9f(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rax,%rcx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2690>
sub    %rax,0x200(%r8)
decq   0x208(%r8)
cmp    $0x3e,%dil
mov    (%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a51>
mov    0x10(%r8,%rdi,8),%rax
mov    %rax,(%rcx)
mov    %rcx,0x10(%r8,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x26d9>
mov    (%rsp),%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x26be>
mov    $0x180,%eax
cmp    $0x19,%r13w
mov    (%rsp),%rdi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x26be>
mov    $0x200,%eax
cmp    $0x21,%r13w
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x26be>
mov    %r13d,%eax
shl    $0x4,%eax
add    $0x30,%eax
and    $0xffffffc0,%eax
lock sub %rax,0x200(%r8)
lock decq 0x208(%r8)
call   *0x18b1e4(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x8(%rsp),%r8
mov    0x20(%rsp),%r10
dec    %r13d
mov    %r13w,0x60(%rbp,%rbx,2)
mov    %r12d,%eax
shr    $0x3,%eax
mov    $0xfffffffffffffffe,%rdx
mov    %r12d,%ecx
rol    %cl,%rdx
and    $0x18,%eax
and    %rdx,0x0(%rbp,%rax,1)
movdqu 0x0(%rbp),%xmm0
movdqu 0x10(%rbp),%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x1,%xmm2
movdqa -0x20c5dd(%rip),%xmm3        # ADDR <anon.ADDR.407.llvm.ADDR+0x10>
pand   %xmm3,%xmm2
psubb  %xmm2,%xmm1
movdqa -0x20c33d(%rip),%xmm2        # ADDR <anon.ADDR.407.llvm.ADDR+0x2c0>
movdqa %xmm1,%xmm4
pand   %xmm2,%xmm4
psrlw  $0x2,%xmm1
pand   %xmm2,%xmm1
paddb  %xmm4,%xmm1
movdqa %xmm1,%xmm4
psrlw  $0x4,%xmm4
paddb  %xmm1,%xmm4
movdqa -0x20c6b7(%rip),%xmm1        # ADDR <anon.ADDR.283.llvm.ADDR+0xf6>
pand   %xmm1,%xmm4
movdqa %xmm0,%xmm5
psrlw  $0x1,%xmm5
pand   %xmm3,%xmm5
psubb  %xmm5,%xmm0
movdqa %xmm0,%xmm3
pand   %xmm2,%xmm3
psrlw  $0x2,%xmm0
pand   %xmm2,%xmm0
paddb  %xmm3,%xmm0
movdqa %xmm0,%xmm2
psrlw  $0x4,%xmm2
paddb  %xmm0,%xmm2
pand   %xmm1,%xmm2
paddb  %xmm4,%xmm2
pxor   %xmm0,%xmm0
psadbw %xmm0,%xmm2
pshufd $0xee,%xmm2,%xmm1
paddq  %xmm2,%xmm1
movd   %xmm1,%ecx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2801>
cmp    $0x7,%r14b
mov    0x30(%rsp),%rsi
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x27e0>
mov    0x8(%rsi),%rax
lea    -0x1e7d2e(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r14,8),%rdx
mov    %rdx,%rdi
and    %rax,%rdi
dec    %rdi
xor    %rax,%rdi
and    %rdx,%rdi
xor    %rax,%rdi
mov    %rdi,0x8(%rsi)
mov    $0x1,%eax
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    %r10,%rbx
mov    %r8,%rdi
call   ADDR <expanse_trie::mutate::downgrade_b_to_l7::<false>>
mov    %rbx,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x164c>
addq   $0xffffffffffffff80,0x200(%r8)
decq   0x208(%r8)
mov    0x28(%r8),%rax
mov    %rax,0x0(%rbp)
mov    %rbp,0x28(%r8)
mov    0x30(%rsp),%rax
movdqu %xmm0,(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x164c>
lea    -0x1e7d59(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8d5>
lea    0x181890(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x508>
mov    $0x6,%esi
call   *0x18b215(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x18181e(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4a8>
mov    $0x8,%edx
xor    %edi,%edi
mov    %rbp,%rsi
call   *0x18b07e(%rip)        # ADDR <_DYNAMIC+0x250>
lea    -0x1e7d83(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8db>
lea    0x181878(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x520>
mov    $0xa1,%esi
call   *0x18b08d(%rip)        # ADDR <_DYNAMIC+0x278>
lea    -0x20adc2(%rip),%rdi        # ADDR <core::unicode::unicode_data::cf::SHORT_OFFSET_RUNS+0xf8>
lea    0x1817cf(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x490>
mov    $0x4,%esi
call   *0x18b1cc(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x181895(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x568>
mov    $0x7,%edx
xor    %edi,%edi
mov    %r15,%rsi
call   *0x18b035(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x181866(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x550>
mov    $0x7,%edx
mov    %rsi,%rdi
mov    %rax,%rsi
call   *0x18b01d(%rip)        # ADDR <_DYNAMIC+0x250>
lea    -0x1e7d94(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x1818d7(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5e0>
mov    $0x12,%esi
call   *0x18b184(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x1e7dad(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x181876(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x598>
mov    $0x12,%esi
call   *0x18b16b(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x181804(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x538>
mov    %r15,%rdi
mov    %rcx,%rsi
call   *0x18b1c8(%rip)        # ADDR <_DYNAMIC+0x440>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0x18aff8(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x1891e1(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f38>
mov    $0x7,%edx
xor    %edi,%edi
call   *0x18afb4(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1891cd(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f38>
mov    $0x7,%edx
xor    %edi,%edi
mov    %r12,%rsi
call   *0x18af9d(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1891ce(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f50>
mov    $0x7,%edx
call   *0x18af8b(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1891bc(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f50>
mov    $0x7,%edx
mov    %r14,%rdi
mov    %r12,%rsi
call   *0x18af73(%rip)        # ADDR <_DYNAMIC+0x250>
lea    -0x1e7f7c(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc7ed>
lea    0x181815(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5c8>
mov    $0x2b,%esi
call   *0x18af82(%rip)        # ADDR <_DYNAMIC+0x278>
lea    -0x1e7f95(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc7ed>
lea    0x1817b4(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x580>
mov    $0x2b,%esi
call   *0x18af69(%rip)        # ADDR <_DYNAMIC+0x278>
lea    0x18181a(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5f8>
mov    $0x7,%esi
mov    $0x7,%edx
mov    %r15,%rdi
call   *0x18af27(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1817b8(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5b0>
mov    $0x7,%esi
mov    $0x7,%edx
mov    %r15,%rdi
call   *0x18af0d(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x18042e(%rip),%rcx        # ADDR <anon.ADDR.892.llvm.ADDR>
mov    $0x1,%edi
mov    %rdx,%rsi
mov    $0x7,%edx
call   *0x18aef3(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1803b4(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
call   *0x18ae9e(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1815c7(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
call   *0x18ae8c(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1816a5(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4f0>
mov    $0x20,%esi
call   *0x18ae7a(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1813bb(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
mov    %rax,%rdi
call   *0x18ae65(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x181606(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x478>
call   *0x18b068(%rip)        # ADDR <_DYNAMIC+0x418>
lea    -0x1e7fb9(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc883>
lea    0x181562(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x3e8>
mov    $0xe,%esi
call   *0x18b007(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x181380(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
call   *0x18ae2d(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x181616(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4c0>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0x18ae16(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x181387(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r14,%rdi
call   *0x18ae01(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x181372(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
call   *0x18b004(%rip)        # ADDR <_DYNAMIC+0x418>
lea    0x18154d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x430>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0x18addd(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x18134e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r15,%rdi
call   *0x18adc8(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1815e1(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4f0>
mov    $0x20,%esi
mov    %r8,%rdi
call   *0x18adb3(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x181324(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r13,%rdi
call   *0x18ad9e(%rip)        # ADDR <_DYNAMIC+0x208>
ud2
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
