push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x4a8,%rsp
mov    %ecx,%r14d
mov    %rsi,%r12
movzbl 0xf(%rsi),%r10d
movzwl %r10w,%eax
cmp    $0xd,%r10w
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe4>
mov    %eax,%ecx
lea    -0x1e9ae0(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xa11>
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
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a7b>
lea    -0x1ddc8e(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
cmp    $0x1,%r10b
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x656>
test   %ax,%ax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
movzbl 0x4(%rbp),%ecx
lea    0x8(%rbp),%rbx
cmp    $0x4,%cl
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xb84>
test   %cl,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfae>
cmp    %dil,(%rbx)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfae>
xor    %r14d,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfda>
cmp    $0x7f,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b>
mov    %eax,%esi
shr    $0x4,%esi
and    $0xf,%eax
mov    %rax,%rbx
lea    0x1(%rax),%ecx
mov    %ecx,%eax
mul    %sil
test   %r10b,%r10b
js     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ac1>
test   %sil,%sil
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ac1>
cmp    $0x10,%al
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ac1>
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
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xa0d>
dec    %esi
cmp    $0x6,%esi
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1523>
lea    -0x1e99f9(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc2d>
movslq (%rax,%rsi,4),%rcx
add    %rax,%rcx
jmp    *%rcx
movzbl 0x0(%rbp),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15d0>
mov    $0x4,%r15b
mov    %r14d,%eax
sub    %r15b,%al
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x252>
mov    %rdi,%r8
movzbl %al,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a7b>
lea    -0x1dddc9(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
mov    %rdx,%r9
cmpb   $0x0,0xf(%r14)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xc50>
mov    $0x1,%eax
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    0x8(%r12),%rcx
movzbl %r13b,%edx
lea    -0x1dde62(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rsi,%rdx,8),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x10b8>
mov    $0x7,%r15b
mov    %r14d,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18a>
mov    0x8(%r12),%rbp
lea    0x0(,%r15,8),%eax
movzbl %al,%r13d
movzbl %r15b,%r8d
lea    -0x1dde96(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%r8,8),%rax
mov    %rax,0x40(%rsp)
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
mov    %r10d,0x18(%rsp)
mov    %rdi,0x8(%rsp)
mov    %rdx,(%rsp)
lea    (%rdx,%rax,8),%rdi
mov    %r8,0x10(%rsp)
mov    %r8d,%edx
call   ADDR <expanse_trie::leaf::locate>
test   $0x1,%al
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x304>
xor    %eax,%eax
mov    %r9,%rdx
add    $0x4a8,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
test   %r14b,%r14b
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ab4>
mov    %rdx,%rbx
mov    (%rsp),%rdx
mov    (%rdx,%rbx,8),%r9
mov    $0x7,%al
movzbl %al,%eax
div    %r14b
movzbl %al,%eax
lea    0x1(%rbp),%r10
cmp    %rax,%r10
mov    0x8(%rsp),%rcx
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x366>
cmp    $0x3,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x689>
mov    $0x2,%r13d
cmp    $0x3,%r10
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11bc>
mov    $0x1,%r13d
cmp    $0x2,%r10
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11bc>
mov    0x10(%rsp),%rax
lea    0x8(%rax),%r11
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x386>
cmp    $0x3,%r10
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x6c1>
mov    0x10(%rsp),%rax
lea    0x8(%rax),%r11
imul   %r10,%r11
test   %rbp,%rbp
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x6d4>
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12)
lea    0xf(%r11),%rax
and    $0xfffffffffffffff0,%rax
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
lea    -0x1dbeb9(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%r11,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe6d>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ada>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c73>
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
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x43c>
sub    %r14b,%al
movzbl %al,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a7b>
lea    -0x1de044(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
mov    %rdx,%r9
mov    0x20(%rbp,%rbx,8),%rsi
mov    %r15d,%edi
shl    $0x4,%edi
cmpb   $0x0,0xf(%rsi,%rdi,1)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x10d4>
mov    $0x1,%eax
cmp    $0x7,%r14b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    0x30(%rsp),%rdi
mov    0x8(%rdi),%rcx
lea    -0x1de133(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x567>
mov    %rdi,%rsi
lea    -0x1(%r14),%eax
movzbl %al,%edi
cmp    $0x8,%r14b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a7b>
lea    -0x1de173(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
shl    $0x8,%rax
mov    0x8(%r12),%rcx
xor    %rdx,%rcx
test   %rax,%rcx
mov    %rsi,%rdi
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    (%r12),%r15
mov    %edx,%ebx
shr    $0x5,%ebx
and    $0x7,%ebx
mov    (%r15,%rbx,4),%eax
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
imul   $0x1010101,%ecx,%r10d
shr    $0x18,%r10d
mov    0x20(%r15,%rbx,8),%r8
mov    (%r8,%r10,8),%r9
lea    -0x1(%rax),%ecx
mov    %eax,%esi
xor    %ecx,%esi
cmp    %ecx,%esi
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xb2a>
movq   $0x0,0x20(%r15,%rbx,8)
lock subq $0x10,0x200(%rdi)
lock decq 0x208(%rdi)
mov    0x40(%rdi),%rax
mov    %rax,(%r8)
mov    %r8,0x40(%rdi)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1bfc>
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
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xbe9>
test   %cl,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x102e>
cmp    %dil,(%rbx)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x102e>
xor    %r14d,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x105a>
cmp    $0x11,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xb06>
lea    0x3(%rbp),%r13d
and    $0x3c,%r13d
cmp    $0x11,%r10
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11bc>
mov    %ebp,%eax
and    $0x1c,%eax
add    $0x4,%eax
cmp    %eax,%r13d
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1119>
lea    0x4(%rbp),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x113e>
lea    0x4(%rbp),%eax
and    $0x3c,%eax
mov    0x10(%rsp),%rcx
lea    0x8(%rcx),%r11
imul   %rax,%r11
mov    %r14d,%eax
sub    %r15b,%al
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x71f>
movzbl %al,%edi
dec    %rdi
mov    0x10(%rsp),%rsi
lea    (%rsi,%r12,1),%rcx
add    $0x7,%rcx
xor    $0x7,%rsi
xor    %eax,%eax
mov    %edi,%edx
nopw   0x0(%rax,%rax,1)
cmp    %esi,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a60>
mov    %rax,%r8
shl    $0x8,%r8
movzbl 0x1(%rcx,%rdi,1),%eax
or     %r8,%rax
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x700>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x721>
xor    %eax,%eax
mov    %r11,0x40(%rsp)
mov    %r14,0x18(%rsp)
mov    %r9,0x20(%rsp)
mov    %r12,0x30(%rsp)
movabs $0x1ffffffffffffffc,%rdx
and    %rbp,%rdx
add    $0x4,%rdx
mov    %r13d,%ecx
shl    %cl,%rax
cmp    $0x3,%r10
pxor   %xmm0,%xmm0
movdqa %xmm0,0x100(%rsp)
movdqa %xmm0,0xf0(%rsp)
movdqa %xmm0,0xe0(%rsp)
movdqa %xmm0,0xd0(%rsp)
movdqa %xmm0,0xc0(%rsp)
movdqa %xmm0,0xb0(%rsp)
movdqa %xmm0,0xa0(%rsp)
movdqa %xmm0,0x90(%rsp)
cmovb  %r10,%rdx
test   %r10,%r10
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x976>
lea    0x0(,%rdx,8),%rcx
lea    0x0(,%rdx,4),%rsi
mov    %rsi,0x38(%rsp)
lea    0x2(%rcx),%rsi
mov    %rsi,0x48(%rsp)
lea    0x4(%rcx),%rsi
mov    %rsi,0x58(%rsp)
lea    0x2(,%rdx,4),%rsi
mov    %rsi,0x88(%rsp)
add    %rdx,%rdx
mov    %rcx,0x50(%rsp)
add    $0x6,%rcx
mov    %rcx,0x80(%rsp)
neg    %rbx
xor    %r13d,%r13d
lea    0x1(%rbp),%rdi
xor    %r15d,%r15d
mov    0x50(%rsp),%rcx
lea    (%rcx,%r13,1),%r8
mov    0x38(%rsp),%rcx
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
mov    0x48(%rsp),%r14
add    %r10,%r14
add    0x58(%rsp),%rsi
add    0x88(%rsp),%r10
neg    %rdi
nopw   0x0(%rax,%rax,1)
lea    (%rbx,%r13,1),%r12
cmp    $0x1,%r12
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x898>
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
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x860>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x976>
mov    0x10(%rsp),%rdi
dec    %edi
lea    -0x1ea152(%rip),%r12        # ADDR <anon.ADDR.24.llvm.ADDR+0xc11>
movslq (%r12,%rdi,4),%rdi
add    %r12,%rdi
jmp    *%rdi
mov    (%rsp),%rsi
movzbl (%rsi,%r8,1),%ecx
mov    %rsi,%r8
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x936>
mov    (%rsp),%r8
mov    -0x4(%r8,%rsi,1),%edi
movzbl (%r8,%rsi,1),%ecx
shl    $0x20,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x933>
mov    (%rsp),%r8
movzwl -0x2(%r8,%r14,1),%esi
movzbl (%r8,%r14,1),%ecx
shl    $0x10,%ecx
or     %rsi,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x936>
mov    (%rsp),%r8
mov    (%r8,%r9,4),%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x936>
mov    (%rsp),%r8
movzwl (%r8,%r11,2),%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x936>
mov    (%rsp),%r8
mov    -0x4(%r8,%r10,2),%esi
movzwl (%r8,%r10,2),%ecx
shl    $0x20,%rcx
or     %rsi,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x936>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2aec>
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
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x801>
cmp    $0x9,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2899>
movzbl 0x18(%rsp),%edx
lea    0x90(%rsp),%rcx
mov    0x8(%rsp),%rbx
mov    %rbx,%rdi
mov    0x30(%rsp),%rsi
mov    %rbp,%r8
call   ADDR <expanse_trie::mutate_map::write_map_immed::<false>>
mov    (%rsp),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28c9>
mov    0x40(%rsp),%rdx
lea    0xf(%rdx),%rax
and    $0xfffffffffffffff0,%rax
lock sub %rax,0x200(%rbx)
lock decq 0x208(%rbx)
cmp    $0x177,%rdx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xa05>
lea    -0x1dc4e6(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xa05>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ada>
mov    0x10(%rbx,%rdi,8),%rax
mov    %rax,(%rcx)
mov    %rcx,0x10(%rbx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x199f>
mov    %rcx,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1999>
mov    %r10d,0x18(%rsp)
mov    %rdi,0x8(%rsp)
movzbl %sil,%eax
mov    %rbp,%rdi
mov    %r12,%rsi
mov    %rdx,%rcx
mov    %eax,%edx
call   ADDR <expanse_trie::leaf::locate>
test   $0x1,%al
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    0x0(%r13),%rax
mov    (%rax,%rdx,8),%r8
cmp    $0x2,%r12d
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe81>
mov    $0x1,%esi
sub    %rdx,%rsi
mov    (%rax,%rsi,8),%rbx
movl   $0x0,0x73(%rsp)
movl   $0x0,0x70(%rsp)
and    $0xf,%r15d
cmp    $0x8,%r15d
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2907>
mov    %rax,%r12
imul   %r15,%rsi
mov    $0x2,%eax
sub    %rdx,%rax
imul   %r15,%rax
mov    %rax,%rcx
sub    %rsi,%rcx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x291e>
cmp    $0x8,%rax
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x291e>
cmp    %r15,%rcx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2968>
mov    %r8,%r14
add    %rbp,%rsi
lea    0x70(%rsp),%rdi
mov    %r15,%rdx
call   *0x18c8fb(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbx,0x0(%r13)
mov    0x70(%rsp),%eax
mov    0x73(%rsp),%ecx
mov    %eax,0x0(%rbp)
mov    %ecx,0x3(%rbp)
mov    0x18(%rsp),%eax
and    $0x70,%al
mov    %al,0xf(%r13)
mov    0x8(%rsp),%rcx
lock subq $0x10,0x200(%rcx)
lock decq 0x208(%rcx)
mov    0x80(%rcx),%rax
mov    %rax,(%r12)
mov    %r12,0x80(%rcx)
mov    $0x1,%eax
mov    %r14,%r9
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
cmp    $0x19,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xdec>
mov    $0x18,%eax
mov    $0x18,%r13d
cmp    $0x19,%r10
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x113e>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11bc>
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
lea    -0x1(%rsi),%r11
mov    %r11,%rax
cmp    $0x3,%r11
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1603>
cmp    $0x11,%r11d
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15ec>
lea    0x2(%rsi),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1603>
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
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfda>
lea    0x182f3d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x448>
mov    $0x3,%esi
mov    %r14,%rdi
call   *0x18c7b7(%rip)        # ADDR <_DYNAMIC+0x208>
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
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x105a>
lea    0x182ea8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x418>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0x18c750(%rip)        # ADDR <_DYNAMIC+0x208>
mov    0x4(%rbx),%eax
xor    %edx,%edx
sub    $0x1,%rax
cmovae %rax,%rdx
cmp    $0xa0,%rdx
mov    %r15,%rdi
mov    %r13,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe10>
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
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe10>
add    $0x100,%rax
cmp    $0x113f,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xc7b>
mov    %edx,0x4(%rbx)
test   %rdx,%rdx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe13>
lock subq $0x1040,0x200(%rdi)
lock decq 0x208(%rdi)
mov    %rbx,%rdi
mov    %r9,%rbx
call   *0x18c5e4(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%r9
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1745>
cmp    $0x21,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1126>
mov    $0x20,%eax
mov    $0x20,%r13d
cmp    $0x21,%r10
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x113e>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11bc>
mov    %edx,0x4(%rbx)
cmp    $0x7,%r8b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe44>
mov    0x8(%r12),%rax
movzbl %r8b,%ecx
lea    -0x1dea51(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
movzbl %r8b,%edx
mov    %r12,%rsi
mov    %r9,%rbx
call   ADDR <expanse_trie::mutate::downgrade_u_to_b::<false>>
mov    %rbx,%r9
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c73>
mov    %rdx,%rdi
mov    %r9,%rbx
call   *0x18c54f(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%r9
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c73>
mov    %rax,0x10(%rsp)
and    $0xf,%r15d
lea    -0x1(%r12),%rcx
lea    0x2(%r12),%eax
and    $0xfffffffc,%eax
cmp    $0x3,%bl
mov    %rcx,0x40(%rsp)
cmovb  %rcx,%rax
lea    0x3(%r12),%esi
and    $0xfffffffc,%esi
cmp    $0x2,%bl
cmovb  %r12,%rsi
cmp    %rsi,%rax
mov    0x8(%rsp),%rcx
mov    %rdx,(%rsp)
mov    %r8,0x20(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x152a>
mov    0x0(%rbp),%eax
mov    0x3(%rbp),%ecx
mov    %ecx,0x63(%rsp)
mov    %eax,0x60(%rsp)
mov    %r12,%rsi
imul   %r15,%rsi
cmp    $0x8,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x298b>
mov    %rdx,%rax
lea    0x1(%rdx),%rbx
mov    %rbx,%rdi
imul   %r15,%rdi
mov    %rsi,%rdx
sub    %rdi,%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29b6>
imul   %r15,%rax
mov    %rdx,%rcx
xor    $0x7,%rcx
cmp    %rcx,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29e0>
lea    (%rsp,%rdi,1),%rsi
add    $0x60,%rsi
lea    (%rsp,%rax,1),%rdi
add    $0x60,%rdi
call   *0x18c554(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    -0x1(%r12),%r14
imul   %r14,%r15
cmp    $0x7,%r15
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a12>
lea    (%rsp,%r15,1),%rdi
add    $0x60,%rdi
xor    $0x7,%r15
xor    %esi,%esi
mov    %r15,%rdx
call   *0x18c54a(%rip)        # ADDR <memset@GLIBC_2.2.5>
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2936>
cmp    %r12,%rbx
mov    0x40(%rsp),%r15
mov    (%rsp),%rcx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xf8a>
mov    0x10(%rsp),%rax
lea    (%rax,%rcx,8),%rdi
lea    (%rax,%rbx,8),%rsi
mov    %r15,%rdx
sub    %rcx,%rdx
shl    $0x3,%rdx
call   *0x18c4f6(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x60(%rsp),%eax
mov    0x63(%rsp),%ecx
mov    %ecx,0x3(%rbp)
mov    %eax,0x0(%rbp)
mov    0x18(%rsp),%ecx
and    $0x70,%cl
lea    -0x1(%r15),%eax
or     %cl,%al
mov    %al,0xf(%r13)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x199f>
cmp    $0x2,%cl
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfbf>
mov    $0x1,%r14d
cmp    %dil,0x9(%rbp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfda>
xor    %eax,%eax
cmp    $0x3,%cl
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    $0x2,%r14d
cmp    %dil,0xa(%rbp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    %r10d,0x18(%rsp)
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
mov    %rdx,%r9
cmpb   $0x0,0xf(%r15)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1099>
mov    %r9,%r15
lea    0x10(%rbp),%rsi
movzbl 0x4(%rbp),%ecx
mov    $0x3,%edx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x142e>
cmp    $0x2,%cl
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x103f>
mov    $0x1,%r14d
cmp    %dil,0x9(%rbp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x105a>
xor    %eax,%eax
cmp    $0x3,%cl
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    $0x2,%r14d
cmp    %dil,0xa(%rbp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    %r10d,0x18(%rsp)
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
mov    %rdx,%r9
cmpb   $0x0,0xf(%r15)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x141e>
mov    $0x1,%eax
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    0x8(%r12),%rcx
lea    -0x1decdc(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r13,8),%rdx
mov    %rdx,%rsi
and    %rcx,%rsi
dec    %rsi
xor    %rcx,%rsi
and    %rdx,%rsi
xor    %rcx,%rsi
mov    %rsi,0x8(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
movzwl 0x60(%rbp,%rbx,2),%r13d
cmp    $0x1,%r13
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1754>
movq   $0x0,0x20(%rbp,%rbx,8)
mov    0x8(%rsp),%r8
lock subq $0x10,0x200(%r8)
lock decq 0x208(%r8)
mov    0x80(%r8),%rax
mov    %rax,(%rsi)
mov    %rsi,0x80(%r8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2748>
lea    0x3(%rbp),%r13d
and    $0x3c,%r13d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11bc>
lea    0x3(%rbp),%r13
and    $0xfffffffffffffffc,%r13
mov    %rbp,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
cmp    %rax,%r13
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11bc>
lea    (%rdx,%rbx,8),%rdi
lea    (%rdx,%rax,8),%r15
lea    0x1(%rbx),%r14
lea    (%rdx,%rbx,8),%rsi
add    $0x8,%rsi
mov    %r12,0x30(%rsp)
mov    %rbp,%r12
sub    %rbx,%r12
lea    0x0(,%r12,8),%rdx
mov    0x18c314(%rip),%r13        # ADDR <memmove@GLIBC_2.2.5>
mov    %r9,0x20(%rsp)
call   *%r13
mov    0x10(%rsp),%rax
imul   %rax,%r14
add    %r15,%r14
imul   %rax,%rbx
add    %r15,%rbx
imul   %rax,%r12
mov    %rbx,%rdi
mov    %r14,%rsi
mov    %r12,%rdx
call   *%r13
mov    0x20(%rsp),%r9
dec    %rbp
mov    0x30(%rsp),%rcx
mov    0x8(%rcx),%rax
xor    %rax,%rbp
and    0x40(%rsp),%rbp
xor    %rax,%rbp
mov    %rbp,0x8(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c73>
mov    0x10(%rsp),%rax
lea    0x8(%rax),%rdx
imul   %rdx,%r13
lea    0xf(%r13),%r15
and    $0xfffffffffffffff0,%r15
lock add %r15,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
cmp    $0x177,%r13
mov    %r9,0x20(%rsp)
mov    %rdx,0x50(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x123a>
lea    -0x1dcd0d(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl 0x0(%r13,%rax,1),%r14d
cmp    $0xff,%r14
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x126c>
cmp    $0x3d,%r14b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b03>
mov    0x10(%rcx,%r14,8),%rsi
test   %rsi,%rsi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x12b2>
mov    (%rsi),%rax
mov    %rax,0x10(%rcx,%r14,8)
mov    %rsi,%r15
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x133d>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r13
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28e2>
mov    $0x1,%esi
mov    %r13,%rdi
call   *0x18c23d(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r15
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x134b>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a6d>
cmp    $0xf,%r13
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x124d>
movq   $0x0,0x90(%rsp)
lea    0x90(%rsp),%rdi
mov    $0x10,%esi
mov    %r13,%rdx
call   *0x18c1fc(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a6d>
mov    0x90(%rsp),%r15
test   %r15,%r15
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x133d>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a6d>
cmp    $0x101,%r13
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x124d>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x297b>
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rcx
mov    %rcx,(%rax)
mov    %r14,0x8(%rax)
mov    %rax,0x220(%rdi)
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b18>
mov    %rax,%r8
cmp    $0x7e0,%r15
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1336>
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
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x130f>
add    $0x40,%r8
mov    %r8,%r15
mov    %r15,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0x18c155(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r12,0x30(%rsp)
lea    0x0(,%rbx,8),%rdx
mov    0x18c051(%rip),%r13        # ADDR <memcpy@GLIBC_2.14>
mov    %r15,%rdi
mov    (%rsp),%r14
mov    %r14,%rsi
call   *%r13
lea    0x1(%rbx),%rax
mov    %rax,0x38(%rsp)
lea    (%r14,%rbx,8),%rsi
add    $0x8,%rsi
mov    %r15,%r12
lea    (%r15,%rbx,8),%rdi
mov    %rbp,%rax
sub    %rbx,%rax
mov    %rax,0x48(%rsp)
lea    0x0(,%rax,8),%rdx
call   *%r13
mov    $0x1,%ecx
mov    $0x2,%eax
lea    0x1(%rbp),%rdx
cmp    $0x3,%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x186f>
cmp    $0x11,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x13cd>
cmp    $0x3,%rdx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x13e8>
mov    $0x2,%ecx
mov    $0x4,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x186f>
cmp    $0x19,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x13f9>
mov    $0x18,%eax
mov    $0x18,%ecx
cmp    $0x11,%rdx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x13ee>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x186f>
lea    0x4(%rbp),%eax
and    $0x3c,%eax
lea    0x3(%rbp),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x186f>
cmp    $0x21,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1686>
xor    %eax,%eax
cmp    $0x19,%rdx
setne  %al
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x186f>
mov    %r9,%r15
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x169b>
add    $0xf6,%al
cmp    $0xf8,%al
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b3a>
movzbl (%rbx),%ecx
and    $0xf,%cl
mov    $0x1,%eax
shl    %cl,%eax
cmp    $0x1,%dl
mov    0x8(%rsp),%rdi
mov    %r15,%r9
mov    0x18(%rsp),%r8d
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16aa>
movzbl 0x9(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x2,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16aa>
movzbl 0xa(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x3,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16aa>
movzbl 0xb(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x4,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16aa>
movzbl 0xc(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x5,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16aa>
movzbl 0xd(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x6,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16aa>
movzbl 0xe(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x7,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x169d>
movzbl 0xf(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x169d>
mov    0x8(%r13),%ecx
movzbl 0xc(%r13),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15ab>
movzwl 0x8(%r13),%ecx
movzbl 0xa(%r13),%eax
shl    $0x10,%eax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15d0>
mov    0x0(%rbp),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15d0>
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15d0>
lea    0x0(,%rax,8),%rdx
lea    0x8(%rdx),%ebx
and    $0x1ff0,%ebx
lock add %rbx,0x200(%rcx)
lock incq 0x208(%rcx)
lock incq 0x218(%rcx)
lea    -0x1dd064(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdi,%rax,8),%r14d
cmp    $0xff,%r14
mov    %rbp,0x30(%rsp)
mov    %rsi,0x50(%rsp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x21a5>
cmp    $0x3d,%r14b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b03>
mov    0x10(%rcx,%r14,8),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x21cf>
mov    0x0(%rbp),%rax
mov    %rax,0x10(%rcx,%r14,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x224f>
movzwl 0x0(%rbp),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15d0>
mov    0x8(%r13),%ecx
movzwl 0xc(%r13),%eax
shl    $0x20,%rax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15d0>
mov    0x8(%r13),%ecx
movzwl 0xc(%r13),%esi
movzbl 0xe(%r13),%eax
shl    $0x20,%rsi
or     %rcx,%rsi
shl    $0x30,%rax
or     %rsi,%rax
cmp    %rdx,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ed>
mov    0x0(%r13),%r9
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%r13)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c73>
mov    $0x18,%eax
cmp    $0x19,%r11d
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1603>
xor    %eax,%eax
cmp    $0x21,%r11d
setb   %al
shl    $0x5,%eax
mov    %rsi,%rcx
cmp    $0x3,%esi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1628>
cmp    $0x11,%esi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1618>
lea    0x3(%rsi),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1628>
xor    %ecx,%ecx
cmp    $0x19,%esi
setae  %cl
lea    0x18(,%rcx,8),%rcx
lea    (%r8,%r10,8),%r13
cmp    %ecx,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1664>
lea    0x8(%r13),%rsi
sub    %r10,%r11
shl    $0x3,%r11
mov    %r14,%rbp
mov    %rdi,%r14
mov    %r13,%rdi
mov    %rdx,%rbx
mov    %r11,%rdx
mov    %r9,%r13
call   *0x18be2d(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r13,%r9
mov    %r14,%rdi
mov    %rbp,%r14
mov    %rbx,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1bfc>
mov    %r11,%rax
cmp    $0x3,%r11d
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1796>
cmp    $0x11,%r11d
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x177f>
lea    0x2(%rsi),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1796>
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x185c>
mov    $0x20,%ecx
mov    $0x24,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x186f>
xor    %eax,%eax
mov    0x8(%rsp),%rdi
mov    %r15,%r9
mov    0x18(%rsp),%r8d
mov    %ax,0x6(%rbp)
mov    (%r12),%rcx
movzbl 0x4(%rcx),%eax
test   %al,%al
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1713>
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16e7>
mov    0x8(%r12),%rcx
lea    -0x1df2f4(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r13,8),%rdx
mov    %rdx,%rsi
and    %rcx,%rsi
dec    %rsi
xor    %rcx,%rsi
and    %rdx,%rsi
xor    %rcx,%rsi
mov    %rsi,0x8(%r12)
cmp    $0x1,%r8b
sete   %cl
cmp    $0x3,%al
setae  %al
or     %cl,%al
mov    $0x1,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
mov    %r12,%rsi
mov    %r9,%rbx
call   ADDR <expanse_trie::mutate::downgrade_l7_to_l3::<false>>
mov    %rbx,%r9
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c73>
xor    %eax,%eax
cmp    $0x1,%r8b
setne  %al
mov    %eax,%edx
shl    $0x6,%edx
add    $0x40,%rdx
lock sub %rdx,0x200(%rdi)
lock decq 0x208(%rdi)
shl    $0x4,%eax
mov    0x18(%rax,%rdi,1),%rdx
mov    %rdx,(%rcx)
mov    %rcx,0x18(%rax,%rdi,1)
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c73>
lea    -0x1(%r13),%rdx
mov    %rdx,%rax
cmp    $0x3,%rdx
mov    0x8(%rsp),%r8
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23e5>
cmp    $0x11,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23ca>
lea    0x2(%r13),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23e5>
mov    $0x18,%eax
cmp    $0x19,%r11d
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1796>
xor    %eax,%eax
cmp    $0x21,%r11d
setb   %al
shl    $0x5,%eax
mov    %r13,0x50(%rsp)
mov    %r14,0x18(%rsp)
lea    0x0(,%rax,8),%rbp
lea    0x8(%rbp),%r14d
and    $0x3f0,%r14d
lock add %r14,0x200(%rdi)
lock incq 0x208(%rdi)
lock incq 0x218(%rdi)
lea    -0x1dd2dc(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rcx,%rax,8),%r13d
cmp    $0xff,%r13
mov    %rdx,0x38(%rsp)
mov    %r9,0x20(%rsp)
mov    %r8,(%rsp)
mov    %rsi,0x40(%rsp)
mov    %r10,0x10(%rsp)
mov    %r11,0x30(%rsp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x182f>
mov    %rbp,0x48(%rsp)
cmp    $0x3d,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b25>
mov    0x10(%rdi,%r13,8),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19bb>
mov    (%rcx),%rax
mov    %rdi,%rbp
mov    %rax,0x10(%rdi,%r13,8)
mov    0x48(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b05>
mov    %rbp,%r14
mov    %rdi,%rbp
mov    $0x10,%esi
mov    %r14,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rdi
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b16>
mov    $0x10,%edi
mov    %r14,%rsi
call   *0x18bbbc(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %rbp,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
lea    0x3(%rbp),%rcx
and    $0xfffffffffffffffc,%rcx
mov    (%rsp),%rdx
lea    (%rdx,%rax,8),%r15
mov    %r12,0x58(%rsp)
lea    (%r12,%rcx,8),%r12
mov    0x10(%rsp),%r13
imul   %r13,%rbx
mov    %r12,%rdi
mov    %r15,%rsi
mov    %rbx,%rdx
mov    0x18bb17(%rip),%r14        # ADDR <memcpy@GLIBC_2.14>
call   *%r14
mov    0x38(%rsp),%rsi
imul   %r13,%rsi
add    %r15,%rsi
add    %r12,%rbx
mov    0x48(%rsp),%rdx
imul   %r13,%rdx
mov    %rbx,%rdi
call   *%r14
lea    0x1(%rbp),%rsi
mov    0x30(%rsp),%rdx
mov    0x18(%rsp),%eax
mov    %al,0xf(%rdx)
lea    -0x1(%rbp),%rax
mov    0x8(%rdx),%rcx
xor    %rcx,%rax
and    0x40(%rsp),%rax
xor    %rcx,%rax
mov    0x58(%rsp),%rcx
mov    %rcx,(%rdx)
mov    %rax,0x8(%rdx)
cmp    $0x3,%rsi
mov    0x8(%rsp),%rcx
mov    0x50(%rsp),%rax
mov    $0x2,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1929>
cmp    $0x11,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x190f>
add    $0x4,%ebp
and    $0x3c,%ebp
mov    %rbp,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1929>
mov    $0x18,%edx
cmp    $0x19,%rsi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1929>
mov    $0x20,%edx
cmp    $0x21,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19ae>
imul   %rax,%rdx
lea    0xf(%rdx),%rax
and    $0xfffffffffffffff0,%rax
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
cmp    $0x177,%rdx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x197f>
lea    -0x1dd45f(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rax,1),%edi
cmp    $0xff,%rdi
mov    (%rsp),%rdx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1996>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ada>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x199f>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%rdx
mov    (%rsp),%rdx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28e2>
mov    %rdx,%rdi
call   *0x18ba29(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    $0x1,%eax
mov    0x20(%rsp),%r9
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
and    $0xfffffffffffffffc,%rbp
add    $0x4,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x190a>
mov    %rdi,%rbp
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x297b>
mov    %rax,%r10
mov    0x220(%rbp),%rax
mov    %rax,(%r10)
mov    %r13,0x8(%r10)
mov    %r10,0x220(%rbp)
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b18>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r14w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a3c>
mov    %rbp,%r9
mov    %rcx,%rax
dec    %rax
mov    %rax,%rsi
imul   %r14,%rsi
lea    (%r10,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x10(%r9,%r13,8),%r8
mov    %r8,0x40(%r10,%rsi,1)
mov    %rdi,0x10(%r9,%r13,8)
dec    %rdx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a16>
mov    %r10,0x58(%rsp)
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
mov    0x48(%rsp),%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1afc>
lea    0xf(%rdx),%r8
shr    $0x4,%r8
lea    -0x4(%rax),%rcx
imul   %r8,%rcx
shl    $0x4,%rcx
add    $0x40,%rcx
lea    -0x3(%rax),%r14
imul   %r8,%r14
lea    -0x2(%rax),%rsi
imul   %r8,%rsi
lea    -0x1(%rax),%rdi
imul   %r8,%rdi
shl    $0x6,%r8
neg    %r8
shl    $0x4,%r14
add    $0x40,%r14
shl    $0x4,%rsi
add    $0x40,%rsi
shl    $0x4,%rdi
add    $0x40,%rdi
mov    0x58(%rsp),%r9
lea    (%r9,%rdi,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x10(%rbp,%r13,8)
lea    (%r9,%rsi,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x10(%rbp,%r13,8)
lea    (%r9,%r14,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%r14,1)
mov    %r10,0x10(%rbp,%r13,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x10(%rbp,%r13,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x10(%rbp,%r13,8)
add    %r8,%r9
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1aa7>
mov    0x58(%rsp),%rcx
add    $0x40,%rcx
mov    %rcx,%rdi
xor    %esi,%esi
mov    %rcx,%r14
call   *0x18b98d(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r14,%rdi
mov    0x10(%rsp),%rax
lea    0x0(,%rax,8),%r13d
mov    (%rsp),%rsi
mov    %r13,%rdx
mov    %rdi,%r14
call   *0x18b87d(%rip)        # ADDR <memcpy@GLIBC_2.14>
add    %r14,%r13
mov    0x50(%rsp),%rsi
add    $0x8,%rsi
mov    0x30(%rsp),%rdx
sub    0x10(%rsp),%rdx
shl    $0x3,%rdx
mov    %r13,%rdi
call   *0x18b85a(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %r14,0x20(%r15,%rbx,8)
mov    0x40(%rsp),%rcx
cmp    $0x3,%ecx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b6c>
mov    0x18(%rsp),%r14
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b8e>
cmp    $0x11,%ecx
mov    0x18(%rsp),%r14
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b7e>
add    $0x3,%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b8e>
xor    %eax,%eax
cmp    $0x19,%ecx
setae  %al
lea    0x18(,%rax,8),%rcx
mov    (%rsp),%rdx
shl    $0x3,%ecx
lea    0x8(%rcx),%rax
and    $0xfffffffffffffff0,%rax
lock sub %rax,0x200(%rbp)
mov    %rbp,%rax
lock decq 0x208(%rbp)
lea    -0x1dd6c1(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rcx,%rsi,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1be3>
cmp    $0x3d,%dil
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ada>
mov    %rax,%rcx
mov    0x10(%rax,%rdi,8),%rax
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
mov    %rcx,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1bf2>
mov    %rdx,%rdi
mov    %rax,%rbx
call   *0x18b7d9(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
mov    0x38(%rsp),%rdx
mov    0x20(%rsp),%r9
mov    %edx,%eax
shr    $0x3,%eax
mov    $0xfffffffffffffffe,%rsi
mov    %edx,%ecx
rol    %cl,%rsi
and    $0x18,%eax
and    %rsi,(%r15,%rax,1)
mov    0x8(%r12),%rax
movzbl %al,%ecx
cmp    $0x15,%rcx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c62>
movzwl 0xd(%r12),%eax
mov    %ax,0x7c(%rsp)
mov    0x9(%r12),%eax
mov    %eax,0x78(%rsp)
cmp    $0x2,%r14b
mov    %rdi,0x8(%rsp)
mov    %r9,0x20(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c7d>
movzbl %r14b,%edx
mov    %r12,%rdi
mov    $0x1,%esi
call   *0x18bac8(%rip)        # ADDR <_DYNAMIC+0x588>
shl    $0x8,%rax
mov    %rax,(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c85>
dec    %rcx
and    $0xffffffffffffff00,%rax
or     %rcx,%rax
mov    %rax,0x8(%r12)
mov    $0x1,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ef>
movq   $0x0,(%rsp)
movq   $0x0,0x290(%rsp)
mov    (%r15),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c9d>
xor    %ecx,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1cc2>
mov    0x8(%r15),%rax
mov    $0x40,%cl
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1cc2>
mov    0x10(%r15),%rax
mov    $0x80,%cl
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1cc2>
mov    0x18(%r15),%rax
mov    $0xc0,%cl
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1deb>
tzcnt  %rax,%rax
or     %cl,%al
lea    0x98(%rsp),%rsi
xor    %edx,%edx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1d01>
data16 cs nopw 0x0(%rax,%rax,1)
shl    $0x6,%dil
tzcnt  %r9,%rax
or     %dil,%al
add    $0x10,%rsi
lea    0x1(%rdx),%rdi
cmp    $0x1f,%rdx
mov    %rdi,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a8d>
movzbl %al,%ecx
mov    %ecx,%edi
shr    $0x5,%dil
mov    $0xffffffff,%r8d
shl    %cl,%r8d
not    %r8d
movzbl %dil,%edi
and    (%r15,%rdi,4),%r8d
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
mov    0x20(%r15,%rdi,8),%rdi
shr    $0x18,%r8d
mov    (%rdi,%r8,8),%rdi
mov    %rcx,-0x8(%rsi)
mov    %rdi,(%rsi)
cmp    $0xff,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1de0>
inc    %al
mov    %eax,%edi
shr    $0x6,%dil
movzbl %dil,%r8d
mov    (%r15,%r8,8),%r9
mov    %eax,%ecx
shr    %cl,%r9
shl    %cl,%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ce0>
lea    0x1(%r8),%rdi
cmp    $0x4,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1de0>
mov    0x8(%r15,%r8,8),%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ce0>
lea    0x2(%r8),%rdi
cmp    $0x4,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1de0>
mov    0x10(%r15,%r8,8),%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ce0>
lea    0x3(%r8),%rdi
cmp    $0x4,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1de0>
mov    0x18(%r15,%r8,8),%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ce0>
inc    %rdx
mov    %rdx,0x290(%rsp)
lea    0x9(%r12),%rax
mov    %rax,0x10(%rsp)
movabs $0x7fffffff0,%rbx
lea    0x2a0(%rsp),%rdi
lea    0x90(%rsp),%rsi
mov    $0x208,%edx
call   *0x18b596(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    $0x4,%ebp
lea    -0x1dd930(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
mov    0x8(%rsp),%rsi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1e68>
mov    %r14,%r13
mov    %rsi,%r14
mov    %r15,%rbx
mov    %r12,%r15
mov    %rdx,%r12
call   *0x18b586(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %r12,%rdx
mov    %r15,%r12
mov    %rbx,%r15
mov    %r14,%rsi
mov    %r13,%r14
movabs $0x7fffffff0,%rbx
inc    %rbp
cmp    $0xc,%rbp
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1f1e>
mov    -0x10(%r15,%rbp,4),%eax
test   %eax,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1e5b>
mov    (%r15,%rbp,8),%rdi
test   %rdi,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25dc>
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
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ed4>
cmp    $0x11,%eax
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ec4>
add    $0x3,%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ed4>
xor    %ecx,%ecx
cmp    $0x19,%eax
setae  %cl
lea    0x18(,%rcx,8),%rax
lea    0x8(,%rax,8),%ecx
and    %ebx,%ecx
lock sub %rcx,0x200(%rsi)
lock decq 0x208(%rsi)
lea    0x0(,%rax,8),%eax
movzbl (%rax,%rdx,1),%eax
cmp    $0xff,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1e2d>
cmp    $0x3d,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a9f>
mov    0x10(%rsi,%rax,8),%rcx
mov    %rcx,(%rdi)
mov    %rdi,0x10(%rsi,%rax,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1e5b>
lock subq $0x80,0x200(%rsi)
lock decq 0x208(%rsi)
mov    0x28(%rsi),%rax
mov    %rax,(%r15)
mov    %r15,0x28(%rsi)
test   %r14b,%r14b
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ab4>
mov    0x4a0(%rsp),%r9
mov    $0x7,%al
movzbl %al,%eax
div    %r14b
movzbl %al,%eax
cmp    %rax,%r9
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x213b>
movq   $0x0,0x290(%rsp)
test   %r9,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2186>
mov    0x2a0(%rsp),%rax
or     (%rsp),%rax
mov    0x2a8(%rsp),%rcx
mov    %rax,0x90(%rsp)
mov    %rcx,0x98(%rsp)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
cmp    $0x1,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2189>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b51>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2189>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b51>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2189>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b51>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2189>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b51>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2189>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b51>
mov    (%rsp),%rcx
or     0x2f0(%rsp),%rcx
mov    0x2f8(%rsp),%rax
shl    $0x4,%r8
mov    %rcx,0x90(%rsp,%r8,1)
mov    %rax,0x98(%rsp,%r8,1)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2189>
lea    0x2a0(%rsp),%rcx
mov    %rsi,%rdi
mov    %r12,%rsi
mov    $0x1,%edx
mov    %r9,%r8
call   ADDR <expanse_trie::mutate_map::build_map_leaf::<false>>
cmp    $0x2,%r14b
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x199f>
movzbl %r14b,%edx
cmp    $0x8,%r14b
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a46>
dec    %rdx
lea    0x78(%rsp),%rsi
mov    0x10(%rsp),%rdi
call   *0x18b22f(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x199f>
xor    %r8d,%r8d
movzbl %r14b,%edx
lea    0x90(%rsp),%rcx
mov    %rsi,%rdi
mov    %r12,%rsi
call   ADDR <expanse_trie::mutate_map::write_map_immed::<false>>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x199f>
mov    $0x10,%esi
mov    %rdx,%rdi
mov    %rdx,%rbx
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rbp
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x225a>
mov    $0x10,%edi
mov    %rbx,%rsi
call   *0x18b249(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %rdx,0x38(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x297b>
mov    %rax,%rbp
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,0x0(%rbp)
mov    %r14,0x8(%rbp)
mov    %rbp,0x220(%rdi)
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b18>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %bx
movzwl %ax,%eax
mov    0x38(%rsp),%rdx
dec    %rax
mov    %rax,%rcx
imul   %rbx,%rcx
lea    (%rcx,%rbp,1),%r8
add    $0x40,%r8
mov    0x10(%rdi,%r14,8),%rsi
mov    %rsi,0x40(%rbp,%rcx,1)
mov    %r8,0x10(%rdi,%r14,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2224>
add    $0x40,%rbp
mov    %rbp,%rdi
xor    %esi,%esi
call   *0x18b246(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    (%rsp),%r8
test   %r8,%r8
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x227d>
lea    0x0(,%r8,8),%rdx
mov    %rbp,%rdi
mov    0x10(%rsp),%rsi
call   *0x18b137(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    (%rsp),%r8
lea    0x1(%r8),%r14
cmp    %r12,%r14
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x22b0>
mov    0x10(%rsp),%rax
lea    (%rax,%r14,8),%rsi
lea    0x0(,%r8,8),%rdi
add    %rbp,%rdi
lea    -0x1(%r12),%rdx
sub    %r8,%rdx
shl    $0x3,%rdx
call   *0x18b104(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    (%rsp),%r8
lea    0x8(%r13),%rcx
mov    (%rcx),%eax
mov    0x3(%rcx),%ecx
mov    %ecx,0x6b(%rsp)
mov    %eax,0x68(%rsp)
imul   %r15,%r12
cmp    $0x8,%r12
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x299f>
imul   %r15,%r14
mov    %r12,%rdx
sub    %r14,%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29c8>
imul   %r15,%r8
mov    %rdx,%rax
xor    $0x7,%rax
cmp    %rax,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29f9>
lea    (%rsp,%r14,1),%rsi
add    $0x68,%rsi
lea    (%rsp,%r8,1),%rdi
add    $0x68,%rdi
call   *0x18b177(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x40(%rsp),%r14
imul   %r14,%r15
cmp    $0x7,%r15
mov    0x10(%rsp),%rbx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a2c>
lea    (%rsp,%r15,1),%rdi
add    $0x68,%rdi
xor    $0x7,%r15
xor    %esi,%esi
mov    %r15,%rdx
call   *0x18b168(%rip)        # ADDR <memset@GLIBC_2.2.5>
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x294f>
mov    %rbp,0x0(%r13)
mov    0x68(%rsp),%eax
mov    0x6b(%rsp),%ecx
mov    0x30(%rsp),%rdx
mov    %ecx,0x3(%rdx)
mov    %eax,(%rdx)
mov    0x18(%rsp),%eax
and    $0x70,%al
dec    %r14b
or     %al,%r14b
mov    %r14b,0xf(%r13)
mov    0x50(%rsp),%rdx
lea    0x8(,%rdx,8),%eax
and    $0xfffffff0,%eax
mov    0x8(%rsp),%rcx
lock sub %rax,0x200(%rcx)
lock decq 0x208(%rcx)
lea    0x0(,%rdx,8),%eax
lea    -0x1ddea3(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rax,%rdx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23c2>
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ada>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0x10(%rcx,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x199f>
mov    %rbx,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1999>
mov    $0x18,%eax
cmp    $0x19,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23e5>
mov    $0x20,%eax
cmp    $0x21,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23e5>
lea    0x2(%r13),%eax
and    $0xfffffffc,%eax
mov    %r13,%rcx
cmp    $0x3,%r13w
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x241e>
cmp    $0x11,%r13w
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23ff>
lea    0x3(%r13),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x241e>
mov    $0x18,%ecx
cmp    $0x19,%r13w
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x241e>
mov    $0x20,%ecx
cmp    $0x21,%r13w
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x241e>
lea    0x3(%r13),%ecx
and    $0xfffffffc,%ecx
add    %rsi,%rdi
cmp    %ecx,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2446>
lea    0x10(%rdi),%rsi
sub    %r15,%rdx
shl    $0x4,%rdx
mov    %r9,%r15
call   *0x18b047(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r15,%r9
mov    0x8(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2748>
mov    %rdx,%rcx
cmp    $0x3,%edx
mov    %r9,0x20(%rsp)
mov    %r13,0x18(%rsp)
mov    %rsi,(%rsp)
mov    %rdi,0x40(%rsp)
mov    %rdx,0x50(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2476>
cmp    $0x11,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24ef>
lea    0x2(%r13),%ecx
and    $0x3c,%ecx
mov    %rcx,%r11
shl    $0x4,%r11
lock add %r11,0x200(%r8)
lea    0x200(%r8),%rax
mov    %rax,0x38(%rsp)
lock incq 0x208(%r8)
lea    0x208(%r8),%r10
lock incq 0x218(%r8)
lea    -0x1ddfb9(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%r11,%rax,1),%r13d
cmp    $0xff,%r13
mov    %r10,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x263e>
mov    %rcx,0x10(%rsp)
cmp    $0x3d,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b25>
mov    0x10(%r8,%r13,8),%rcx
test   %rcx,%rcx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x253e>
mov    (%rcx),%rax
mov    %rax,0x10(%r8,%r13,8)
mov    %rcx,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25c9>
mov    $0x180,%r11d
cmp    $0x19,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2509>
mov    $0x200,%r11d
cmp    $0x21,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25f5>
lock add %r11,0x200(%r8)
lea    0x200(%r8),%rax
mov    %rax,0x38(%rsp)
lock incq 0x208(%r8)
lea    0x208(%r8),%rax
mov    %rax,0x48(%rsp)
lock incq 0x218(%r8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x263e>
cmpl   $0x11,0x10(%rsp)
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x263e>
mov    %r11,0x58(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x297b>
mov    %rax,%rdi
mov    0x8(%rsp),%r8
mov    0x220(%r8),%rax
mov    %rax,(%rdi)
mov    %r13,0x8(%rdi)
mov    %rdi,0x220(%r8)
cmpq   $0x0,0x10(%rsp)
mov    0x58(%rsp),%r11
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2b18>
mov    $0xfc0,%eax
xor    %edx,%edx
div    %r11d
dec    %rax
mov    %rax,%rcx
imul   %r11,%rcx
lea    (%rdi,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x10(%r8,%r13,8),%rsi
mov    %rsi,0x40(%rdi,%rcx,1)
mov    %rdx,0x10(%r8,%r13,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x259e>
add    $0x40,%rdi
xor    %esi,%esi
mov    %r11,%rdx
mov    %rdi,%r13
call   *0x18aec9(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r13,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x265a>
lea    -0x1e01cb(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8d5>
lea    0x1815ee(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x508>
mov    $0x6,%esi
call   *0x18af73(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x2(%r13),%ecx
and    $0xfffffffc,%ecx
mov    %ecx,%r11d
shl    $0x4,%r11d
lea    0x200(%r8),%rax
lock add %r11,0x200(%r8)
lea    0x208(%r8),%r10
lock incq 0x208(%r8)
lock incq 0x218(%r8)
cmp    $0x15,%r13w
mov    %rax,0x38(%rsp)
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24a8>
mov    %r10,0x48(%rsp)
mov    $0x10,%esi
mov    %r11,%rdi
mov    %r11,%r13
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rdi
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a6d>
mov    %rdi,0x10(%rsp)
mov    %r15d,%r13d
shl    $0x4,%r13d
mov    (%rsp),%rsi
mov    %r13,%rdx
call   *0x18ad3d(%rip)        # ADDR <memcpy@GLIBC_2.14>
add    0x10(%rsp),%r13
mov    0x40(%rsp),%rsi
add    $0x10,%rsi
mov    0x50(%rsp),%rdx
sub    %r15,%rdx
shl    $0x4,%rdx
mov    %r13,%rdi
call   *0x18ad1a(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x10(%rsp),%rax
mov    %rax,0x20(%rbp,%rbx,8)
mov    0x18(%rsp),%r13
mov    %r13,%rax
cmp    $0x3,%r13w
mov    0x38(%rsp),%rcx
mov    0x48(%rsp),%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x26c7>
cmp    $0x11,%r13w
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2708>
lea    0x3(%r13),%eax
and    $0x3c,%eax
shl    $0x4,%eax
lock sub %rax,(%rcx)
lock decq (%rdx)
lea    -0x1de1e3(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rax,%rcx,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2734>
cmp    $0x3e,%dil
mov    0x8(%rsp),%r8
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2ada>
mov    0x10(%r8,%rdi,8),%rax
mov    (%rsp),%rcx
mov    %rax,(%rcx)
mov    %rcx,0x10(%r8,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2743>
mov    $0x180,%eax
cmp    $0x19,%r13w
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x272c>
mov    $0x200,%eax
cmp    $0x21,%r13w
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x272c>
mov    %r13d,%eax
shl    $0x4,%eax
add    $0x30,%eax
and    $0xffffffc0,%eax
lock sub %rax,(%rcx)
lock decq (%rdx)
mov    (%rsp),%rdi
call   *0x18ac8a(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x8(%rsp),%r8
mov    0x20(%rsp),%r9
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
movdqa -0x204d07(%rip),%xmm3        # ADDR <anon.ADDR.407.llvm.ADDR+0x10>
pand   %xmm3,%xmm2
psubb  %xmm2,%xmm1
movdqa -0x204a67(%rip),%xmm2        # ADDR <anon.ADDR.407.llvm.ADDR+0x2c0>
movdqa %xmm1,%xmm4
pand   %xmm2,%xmm4
psrlw  $0x2,%xmm1
pand   %xmm2,%xmm1
paddb  %xmm4,%xmm1
movdqa %xmm1,%xmm4
psrlw  $0x4,%xmm4
paddb  %xmm1,%xmm4
movdqa -0x204de1(%rip),%xmm1        # ADDR <anon.ADDR.283.llvm.ADDR+0xf6>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x286b>
cmp    $0x7,%r14b
mov    0x30(%rsp),%rsi
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x284a>
mov    0x8(%rsi),%rax
lea    -0x1e0458(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
mov    %r8,%rdi
mov    %r9,%rbx
call   ADDR <expanse_trie::mutate::downgrade_b_to_l7::<false>>
mov    %rbx,%r9
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c73>
lock subq $0x80,0x200(%r8)
lock decq 0x208(%r8)
mov    0x28(%r8),%rax
mov    %rax,0x0(%rbp)
mov    %rbp,0x28(%r8)
mov    0x30(%rsp),%rax
movdqu %xmm0,(%rax)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c73>
lea    0x1812d8(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4a8>
mov    $0x8,%edx
xor    %edi,%edi
mov    %rbp,%rsi
call   *0x18ab38(%rip)        # ADDR <_DYNAMIC+0x250>
lea    -0x1e0499(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8db>
lea    0x181332(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x520>
mov    $0xa1,%esi
call   *0x18ab47(%rip)        # ADDR <_DYNAMIC+0x278>
lea    -0x2034d8(%rip),%rdi        # ADDR <core::unicode::unicode_data::cf::SHORT_OFFSET_RUNS+0xf8>
lea    0x181289(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x490>
mov    $0x4,%esi
call   *0x18ac86(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x1e05a4(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0x187e80(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x181009(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x2f(%rsp),%rdx
mov    $0x11,%esi
call   *0x18ab59(%rip)        # ADDR <_DYNAMIC+0x2c8>
lea    0x18132a(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x568>
mov    $0x7,%edx
xor    %edi,%edi
mov    %r15,%rsi
call   *0x18aaca(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1812fb(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x550>
mov    $0x7,%edx
mov    %rsi,%rdi
mov    %rax,%rsi
call   *0x18aab2(%rip)        # ADDR <_DYNAMIC+0x250>
lea    -0x1e04cf(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x18136c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5e0>
mov    $0x12,%esi
call   *0x18ac19(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x1e04e8(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x18130b(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x598>
mov    $0x12,%esi
call   *0x18ac00(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x181299(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x538>
mov    %r15,%rdi
mov    %rcx,%rsi
call   *0x18ac5d(%rip)        # ADDR <_DYNAMIC+0x440>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0x18aa8d(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x188c76(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f38>
mov    $0x7,%edx
xor    %edi,%edi
call   *0x18aa49(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x188c62(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f38>
mov    $0x7,%edx
xor    %edi,%edi
mov    %r12,%rsi
call   *0x18aa32(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x188c63(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f50>
mov    $0x7,%edx
call   *0x18aa20(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x188c51(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f50>
mov    $0x7,%edx
mov    %r14,%rdi
mov    %r12,%rsi
call   *0x18aa08(%rip)        # ADDR <_DYNAMIC+0x250>
lea    -0x1e06b7(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc7ed>
lea    0x1812aa(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5c8>
mov    $0x2b,%esi
call   *0x18aa17(%rip)        # ADDR <_DYNAMIC+0x278>
lea    -0x1e06d0(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc7ed>
lea    0x181249(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x580>
mov    $0x2b,%esi
call   *0x18a9fe(%rip)        # ADDR <_DYNAMIC+0x278>
lea    0x1812af(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5f8>
mov    $0x7,%esi
mov    $0x7,%edx
mov    %r15,%rdi
call   *0x18a9bc(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x18124d(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5b0>
mov    $0x7,%esi
mov    $0x7,%edx
mov    %r15,%rdi
call   *0x18a9a2(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x17fec3(%rip),%rcx        # ADDR <anon.ADDR.892.llvm.ADDR>
mov    $0x1,%edi
mov    %rdx,%rsi
mov    $0x7,%edx
call   *0x18a988(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x17fe49(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
call   *0x18a933(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x10,%edi
mov    %r13,%rsi
call   *0x18a99d(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x18104e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
call   *0x18a913(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x18112c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4f0>
mov    $0x20,%esi
call   *0x18a901(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x180e42(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
mov    %rax,%rdi
call   *0x18a8ec(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x18108d(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x478>
call   *0x18aaef(%rip)        # ADDR <_DYNAMIC+0x418>
lea    -0x1e0702(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc883>
lea    0x180fe9(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x3e8>
mov    $0xe,%esi
call   *0x18aa8e(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x180e07(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
call   *0x18a8b4(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x18109d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4c0>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0x18a89d(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x180e0e(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r14,%rdi
call   *0x18a888(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x180df9(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
call   *0x18aa8b(%rip)        # ADDR <_DYNAMIC+0x418>
lea    0x180dec(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r13,%rdi
call   *0x18a866(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x180fbf(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x430>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0x18a84f(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x181068(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4f0>
mov    $0x20,%esi
mov    %r8,%rdi
call   *0x18a83a(%rip)        # ADDR <_DYNAMIC+0x208>
ud2
int3
int3
int3
int3
int3
int3
int3
int3
