push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x4a8,%rsp
mov    %ecx,%ebx
mov    %rsi,%r12
movzbl 0xf(%rsi),%r14d
movzwl %r14w,%eax
cmp    $0xd,%r14w
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe5>
mov    %eax,%ecx
lea    -0x1f0eef(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xa11>
movslq (%rsi,%rcx,4),%rcx
add    %rsi,%rcx
jmp    *%rcx
mov    %rdi,%r8
mov    (%r12),%rbp
movzbl 0x5(%rbp),%r13d
cmp    %r13b,%bl
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x84>
sub    %r13b,%bl
movzbl %bl,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24e4>
lea    -0x1e509c(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
lea    0x0(,%r13,8),%ecx
shl    %cl,%rax
mov    0x8(%r12),%rcx
xor    %rdx,%rcx
test   %rax,%rcx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
lea    -0x1(%r13),%esi
lea    0x0(,%rsi,8),%ecx
mov    %rdx,%rdi
shr    %cl,%rdi
mov    %edi,%ecx
and    $0xf,%cl
mov    $0x1,%eax
shl    %cl,%eax
and    0x6(%rbp),%ax
cmp    $0x1,%r14b
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x621>
test   %ax,%ax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
movzbl 0x4(%rbp),%ecx
lea    0x8(%rbp),%r9
cmp    $0x4,%cl
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xb5e>
mov    %r14d,%ebx
test   %cl,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xf66>
cmp    %dil,(%r9)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xf66>
xor    %r14d,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xf92>
cmp    $0x7f,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a>
mov    %eax,%esi
shr    $0x4,%esi
and    $0xf,%eax
mov    %rax,%rbx
lea    0x1(%rax),%ecx
mov    %ecx,%eax
mul    %sil
test   %r14b,%r14b
js     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2516>
test   %sil,%sil
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2516>
cmp    $0x10,%al
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2516>
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
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x9d7>
dec    %esi
cmp    $0x6,%esi
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x138a>
lea    -0x1f0e0a(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc2d>
movslq (%rax,%rsi,4),%rcx
add    %rax,%rcx
jmp    *%rcx
movzbl 0x0(%rbp),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1434>
mov    $0x4,%r15b
mov    %ebx,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x45f>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x4ac>
mov    %rbx,%r13
lea    -0x1(%rbx),%eax
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    %rdx,%r10
cmpb   $0x0,0xf(%r14)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xc32>
mov    $0x1,%eax
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
mov    0x8(%r12),%rcx
movzbl %r13b,%edx
lea    -0x1e5230(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rsi,%rdx,8),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1065>
mov    $0x7,%r15b
mov    %ebx,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x45f>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x4ac>
mov    $0x2,%r15b
mov    %ebx,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x45f>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x4ac>
mov    %rdi,%r8
mov    (%r12),%r13
movzbl 0x74(%r13),%r14d
cmp    %r14b,%bl
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x26f>
sub    %r14b,%bl
movzbl %bl,%edi
cmp    $0x8,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24e4>
lea    -0x1e5287(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
lea    0x0(,%r14,8),%ecx
shl    %cl,%rax
mov    0x8(%r12),%rcx
xor    %rdx,%rcx
test   %rax,%rcx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
lea    -0x1(%r14),%eax
lea    0x0(,%rax,8),%ecx
mov    %rdx,%r15
shr    %cl,%r15
mov    %r15d,%ebx
shr    $0x5,%ebx
and    $0x7,%ebx
mov    0x0(%r13,%rbx,4),%esi
mov    $0x1,%edi
mov    %r15d,%ecx
shl    %cl,%edi
bt     %r15d,%esi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    %r12,0x30(%rsp)
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
imul   $0x1010101,%ecx,%r12d
shr    $0x18,%r12d
mov    %r12d,%esi
shl    $0x4,%esi
add    0x20(%r13,%rbx,8),%rsi
movzbl %al,%ecx
mov    %r8,%rdi
mov    %r8,0x8(%rsp)
call   ADDR <expanse_trie::mutate_map::map_remove::<false, false>>
cmp    $0x1,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    %rdx,%r10
mov    0x20(%r13,%rbx,8),%rsi
mov    %r12d,%edi
shl    $0x4,%edi
cmpb   $0x0,0xf(%rsi,%rdi,1)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1081>
mov    $0x1,%eax
cmp    $0x7,%r14b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
mov    0x30(%rsp),%rdi
mov    0x8(%rdi),%rcx
lea    -0x1e5377(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r14,8),%rdx
mov    %rdx,%rsi
and    %rcx,%rsi
dec    %rsi
xor    %rcx,%rsi
and    %rdx,%rsi
xor    %rcx,%rsi
mov    %rsi,0x8(%rdi)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
cmp    $0x1,%bl
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x398>
mov    %rdi,%rsi
lea    -0x1(%rbx),%eax
movzbl %al,%edi
cmp    $0x8,%bl
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24e4>
lea    -0x1e53b4(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
shl    $0x8,%rax
mov    0x8(%r12),%rcx
xor    %rdx,%rcx
test   %rax,%rcx
mov    %rsi,%rdi
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    %rbx,0x18(%rsp)
mov    (%r12),%rbx
mov    %edx,%r14d
shr    $0x5,%r14d
and    $0x7,%r14d
mov    (%rbx,%r14,4),%eax
mov    $0x1,%esi
mov    %edx,%ecx
shl    %cl,%esi
bt     %edx,%eax
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
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
mov    0x20(%rbx,%r14,8),%r8
mov    (%r8,%r9,8),%r10
lea    -0x1(%rax),%ecx
mov    %eax,%esi
xor    %ecx,%esi
cmp    %ecx,%esi
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xb04>
movq   $0x0,0x20(%rbx,%r14,8)
addq   $0xfffffffffffffff0,0x200(%rdi)
decq   0x208(%rdi)
mov    0x40(%rdi),%rax
mov    %rax,(%r8)
mov    %r8,0x40(%rdi)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x190b>
mov    $0x1,%r15b
mov    %ebx,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x45f>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x4ac>
mov    $0x5,%r15b
mov    %ebx,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x45f>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x4ac>
mov    $0x3,%r15b
mov    %ebx,%eax
sub    %r15b,%al
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x4ac>
mov    %rdi,%r8
movzbl %al,%edi
cmp    $0x7,%dil
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24e4>
lea    -0x1e54ae(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rax,%rdi,8),%rax
lea    0x0(,%r15,8),%ecx
shl    %cl,%rax
mov    0x8(%r12),%rbp
mov    %rbp,%rsi
xor    %rdx,%rsi
test   %rax,%rsi
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
movzbl %cl,%r13d
mov    %r8,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x4bd>
mov    $0x6,%r15b
mov    %ebx,%eax
sub    %r15b,%al
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x45f>
mov    0x8(%r12),%rbp
lea    0x0(,%r15,8),%eax
movzbl %al,%r13d
movzbl %r15b,%r8d
lea    -0x1e5500(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x524>
cmp    $0x11,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x50e>
mov    %rbp,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x524>
mov    $0x18,%eax
cmp    $0x19,%rsi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x524>
mov    $0x20,%eax
cmp    $0x21,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x501>
mov    %r14d,0x18(%rsp)
mov    %rdi,0x8(%rsp)
mov    %rdx,(%rsp)
lea    (%rdx,%rax,8),%rdi
mov    %r8,0x20(%rsp)
mov    %r8d,%edx
call   ADDR <expanse_trie::leaf::locate>
test   $0x1,%al
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x55e>
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
test   %bl,%bl
mov    0x8(%rsp),%rcx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2957>
mov    %rdx,%r14
mov    (%rsp),%rdx
mov    (%rdx,%r14,8),%r10
mov    $0x7,%al
movzbl %al,%eax
div    %bl
movzbl %al,%eax
lea    0x1(%rbp),%r9
cmp    %rax,%r9
mov    0x20(%rsp),%r8
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x5be>
cmp    $0x3,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x657>
mov    $0x2,%r13d
cmp    $0x3,%r9
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1168>
mov    $0x1,%r13d
cmp    $0x2,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1168>
lea    0x8(%r8),%r11
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x5d9>
cmp    $0x3,%r9
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x68f>
lea    0x8(%r8),%r11
imul   %r9,%r11
test   %rbp,%rbp
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x69d>
pxor   %xmm0,%xmm0
movdqu %xmm0,(%r12)
lea    0xf(%r11),%rsi
and    $0xfffffffffffffff0,%rsi
lea    -0x1e350a(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%r11,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x236a>
sub    %rsi,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c46>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2945>
test   %ax,%ax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
movzbl 0x4(%rbp),%ecx
lea    0x8(%rbp),%r9
cmp    $0x4,%cl
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xbc7>
test   %cl,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfdd>
cmp    %dil,(%r9)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xfdd>
mov    %r14d,%ebx
xor    %r14d,%r14d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x100c>
cmp    $0x11,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xae0>
lea    0x3(%rbp),%r13d
and    $0x3c,%r13d
cmp    $0x11,%r9
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1168>
mov    %ebp,%eax
and    $0x1c,%eax
add    $0x4,%eax
cmp    %eax,%r13d
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x10c8>
lea    0x4(%rbp),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x10ed>
lea    0x4(%rbp),%eax
and    $0x3c,%eax
lea    0x8(%r8),%r11
imul   %rax,%r11
mov    %ebx,%eax
sub    %r15b,%al
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x6df>
movzbl %al,%edi
dec    %rdi
lea    (%r8,%r12,1),%rcx
add    $0x7,%rcx
mov    %r8,%rsi
xor    $0x7,%rsi
xor    %eax,%eax
mov    %edi,%edx
nopl   (%rax)
cmp    %esi,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24d7>
mov    %rax,%r8
shl    $0x8,%r8
movzbl 0x1(%rcx,%rdi,1),%eax
or     %r8,%rax
add    $0xffffffffffffffff,%rdi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x6c0>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x6e1>
xor    %eax,%eax
mov    %r11,0x38(%rsp)
mov    %rbx,0x18(%rsp)
mov    %r10,0x10(%rsp)
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x936>
lea    0x0(,%rdx,8),%rcx
lea    0x0(,%rdx,4),%rsi
mov    %rsi,0x40(%rsp)
lea    0x2(%rcx),%rsi
mov    %rsi,0x60(%rsp)
lea    0x4(%rcx),%rsi
mov    %rsi,0x68(%rsp)
lea    0x2(,%rdx,4),%rsi
mov    %rsi,0x88(%rsp)
add    %rdx,%rdx
mov    %rcx,0x48(%rsp)
add    $0x6,%rcx
mov    %rcx,0x80(%rsp)
neg    %r14
xor    %r13d,%r13d
lea    0x1(%rbp),%rdi
xor    %r15d,%r15d
mov    0x48(%rsp),%rcx
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
mov    0x60(%rsp),%rbx
add    %r10,%rbx
add    0x68(%rsp),%rsi
add    0x88(%rsp),%r10
neg    %rdi
nopw   0x0(%rax,%rax,1)
lea    (%r14,%r13,1),%r12
cmp    $0x1,%r12
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x858>
inc    %r8
inc    %r11
add    $0x3,%rbx
inc    %r9
add    $0x5,%rsi
add    $0x3,%r10
add    $0x7,%rcx
lea    (%rdi,%r13,1),%r12
inc    %r12
inc    %r13
cmp    $0x1,%r12
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x820>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x936>
mov    0x20(%rsp),%rdi
dec    %edi
lea    -0x1f1522(%rip),%r12        # ADDR <anon.ADDR.24.llvm.ADDR+0xc11>
movslq (%r12,%rdi,4),%rdi
add    %r12,%rdi
jmp    *%rdi
mov    (%rsp),%rsi
movzbl (%rsi,%r8,1),%ecx
mov    %rsi,%r8
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x8f6>
mov    (%rsp),%r8
mov    -0x4(%r8,%rsi,1),%edi
movzbl (%r8,%rsi,1),%ecx
shl    $0x20,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x8f3>
mov    (%rsp),%r8
movzwl -0x2(%r8,%rbx,1),%esi
movzbl (%r8,%rbx,1),%ecx
shl    $0x10,%ecx
or     %rsi,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x8f6>
mov    (%rsp),%r8
mov    (%r8,%r9,4),%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x8f6>
mov    (%rsp),%r8
movzwl (%r8,%r11,2),%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x8f6>
mov    (%rsp),%r8
mov    -0x4(%r8,%r10,2),%esi
movzwl (%r8,%r10,2),%ecx
shl    $0x20,%rcx
or     %rsi,%rcx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x8f6>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2544>
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
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x7c1>
cmp    $0x9,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x208a>
movzbl 0x18(%rsp),%edx
lea    0x90(%rsp),%rcx
mov    0x8(%rsp),%rdi
mov    0x30(%rsp),%rsi
mov    %rbp,%r8
call   ADDR <expanse_trie::mutate_map::write_map_immed::<false>>
mov    (%rsp),%rbx
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x20ba>
mov    0x38(%rsp),%rcx
lea    0xf(%rcx),%rsi
and    $0xfffffffffffffff0,%rsi
cmp    $0x177,%rcx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1450>
lea    -0x1e38a5(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rcx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1450>
mov    0x8(%rsp),%rcx
sub    %rsi,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2945>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0x10(%rcx,%rdi,8)
mov    $0x1,%eax
mov    0x10(%rsp),%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
mov    %rdi,0x8(%rsp)
movzbl %sil,%eax
mov    %rbp,%rdi
mov    %r12,%rsi
mov    %rdx,%rcx
mov    %eax,%edx
call   ADDR <expanse_trie::leaf::locate>
test   $0x1,%al
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    %rdx,%r10
mov    0x0(%r13),%r9
mov    (%r9,%rdx,8),%r8
cmp    $0x2,%r12d
mov    0x8(%rsp),%rdi
mov    %r14d,0x18(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe43>
mov    %rbp,%rdx
mov    $0x1,%esi
sub    %r10,%rsi
mov    (%r9,%rsi,8),%rbp
movl   $0x0,0x73(%rsp)
movl   $0x0,0x70(%rsp)
and    $0xf,%r15d
cmp    $0x8,%r15d
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x238a>
imul   %r15,%rsi
mov    $0x2,%eax
sub    %r10,%rax
imul   %r15,%rax
mov    %rax,%rcx
sub    %rsi,%rcx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23a1>
cmp    $0x8,%rax
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23a1>
cmp    %r15,%rcx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2409>
mov    %r9,%rbx
mov    %r8,%r14
mov    %rdx,%r12
add    %rdx,%rsi
lea    0x70(%rsp),%rdi
mov    %r15,%rdx
call   *0x18b923(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,0x0(%r13)
mov    0x70(%rsp),%eax
mov    0x73(%rsp),%ecx
mov    %eax,(%r12)
mov    %ecx,0x3(%r12)
mov    0x18(%rsp),%ebp
and    $0x70,%bpl
mov    %bpl,0xf(%r13)
mov    0x8(%rsp),%rcx
addq   $0xfffffffffffffff0,0x200(%rcx)
decq   0x208(%rcx)
mov    0x80(%rcx),%rax
mov    %rax,(%rbx)
mov    %rbx,0x80(%rcx)
mov    $0x1,%eax
mov    %r14,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
cmp    $0x19,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xdc2>
mov    $0x18,%eax
mov    $0x18,%r13d
cmp    $0x19,%r9
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x10ed>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1168>
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
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1480>
cmp    $0x11,%r11d
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1469>
lea    0x2(%rsi),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1480>
movzbl %cl,%eax
cmp    $0x8,%cl
mov    $0x8,%ecx
cmovb  %eax,%ecx
movzbl %dil,%eax
movd   %eax,%xmm0
punpcklbw %xmm0,%xmm0
pshuflw $0x0,%xmm0,%xmm0
pshufd $0x44,%xmm0,%xmm0
movq   (%r9),%xmm1
mov    $0xffffffff,%eax
shl    %cl,%eax
pcmpeqb %xmm0,%xmm1
pmovmskb %xmm1,%ecx
not    %eax
and    %ecx,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    %r14d,%ebx
tzcnt  %eax,%r14d
cmp    $0x2,%r14d
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xf92>
lea    0x181f5f(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x448>
mov    $0x3,%esi
mov    %r14,%rdi
call   *0x18b7d9(%rip)        # ADDR <_DYNAMIC+0x208>
movzbl %cl,%eax
cmp    $0x8,%cl
mov    $0x8,%ecx
cmovb  %eax,%ecx
movzbl %dil,%eax
movd   %eax,%xmm0
punpcklbw %xmm0,%xmm0
pshuflw $0x0,%xmm0,%xmm0
pshufd $0x44,%xmm0,%xmm0
movq   (%r9),%xmm1
mov    $0xffffffff,%eax
shl    %cl,%eax
pcmpeqb %xmm0,%xmm1
pmovmskb %xmm1,%ecx
not    %eax
and    %ecx,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    %r14d,%ebx
tzcnt  %eax,%r14d
cmp    $0x6,%r14d
jbe    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x100c>
lea    0x181ec6(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x418>
mov    $0x7,%edi
mov    $0x7,%esi
call   *0x18b76e(%rip)        # ADDR <_DYNAMIC+0x208>
mov    0x4(%rbx),%eax
xor    %edx,%edx
sub    $0x1,%rax
cmovae %rax,%rdx
cmp    $0xa0,%rdx
mov    %r15,%rdi
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xde6>
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
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xde6>
add    $0x100,%rax
cmp    $0x113f,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xc5a>
mov    %edx,0x4(%rbx)
test   %rdx,%rdx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xde9>
mov    $0x1040,%esi
mov    %r10,%r14
call   *0x18b91f(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %rbx,%rdi
call   *0x18b60e(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %r14,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16d4>
cmp    $0x21,%rbp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x10d5>
mov    $0x20,%eax
mov    $0x20,%r13d
cmp    $0x21,%r9
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x10ed>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1168>
mov    %edx,0x4(%rbx)
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xe1a>
mov    0x8(%r12),%rax
movzbl %r13b,%ecx
lea    -0x1e5e37(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
movzbl %r13b,%edx
mov    %r12,%rsi
mov    %r10,%rbx
call   ADDR <expanse_trie::mutate::downgrade_u_to_b::<false>>
mov    %rbx,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c53>
and    $0xf,%r15d
lea    -0x1(%r12),%rcx
lea    0x2(%r12),%eax
and    $0xfffffffc,%eax
cmp    $0x3,%bl
mov    %rcx,0x20(%rsp)
cmovb  %rcx,%rax
lea    0x3(%r12),%ecx
and    $0xfffffffc,%ecx
cmp    $0x2,%bl
cmovb  %r12,%rcx
cmp    %rcx,%rax
mov    %r10,(%rsp)
mov    %r8,0x10(%rsp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1391>
mov    0x0(%rbp),%eax
mov    0x3(%rbp),%ecx
mov    %ecx,0x53(%rsp)
mov    %eax,0x50(%rsp)
mov    %r12,%rsi
imul   %r15,%rsi
cmp    $0x8,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x241c>
lea    0x1(%r10),%rbx
mov    %rbx,%rdi
imul   %r15,%rdi
mov    %rsi,%rdx
sub    %rdi,%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2447>
imul   %r15,%r10
mov    %rdx,%rcx
xor    $0x7,%rcx
cmp    %rcx,%r10
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2471>
mov    %r9,%r14
lea    (%rsp,%rdi,1),%rsi
add    $0x50,%rsi
lea    (%rsp,%r10,1),%rdi
add    $0x50,%rdi
call   *0x18b59c(%rip)        # ADDR <memmove@GLIBC_2.2.5>
lea    -0x1(%r12),%rbp
imul   %rbp,%r15
cmp    $0x7,%r15
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24a3>
lea    (%rsp,%r15,1),%rdi
add    $0x50,%rdi
xor    $0x7,%r15
xor    %esi,%esi
mov    %r15,%rdx
call   *0x18b592(%rip)        # ADDR <memset@GLIBC_2.2.5>
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23b9>
cmp    %r12,%rbx
mov    0x20(%rsp),%r15
mov    (%rsp),%rax
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xf3d>
lea    (%r14,%rax,8),%rdi
lea    (%r14,%rbx,8),%rsi
mov    %r15,%rdx
sub    %rax,%rdx
shl    $0x3,%rdx
call   *0x18b543(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x50(%rsp),%eax
mov    0x53(%rsp),%ecx
lea    0x8(%r13),%rdx
mov    %ecx,0x3(%rdx)
mov    %eax,(%rdx)
mov    0x18(%rsp),%ebp
and    $0x70,%bpl
lea    -0x1(%r15),%eax
or     %bpl,%al
mov    %al,0xf(%r13)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x9c8>
cmp    $0x2,%cl
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xf77>
mov    $0x1,%r14d
cmp    %dil,0x9(%rbp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xf92>
xor    %eax,%eax
cmp    $0x3,%cl
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
mov    $0x2,%r14d
cmp    %dil,0xa(%rbp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
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
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    %rdx,%r10
cmpb   $0x0,0xf(%r15)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1046>
mov    %r10,%r15
lea    0x10(%rbp),%rsi
movzbl 0x4(%rbp),%ecx
mov    $0x3,%edx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1295>
mov    %r14d,%ebx
cmp    $0x2,%cl
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0xff1>
mov    $0x1,%r14d
cmp    %dil,0x9(%rbp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x100c>
xor    %eax,%eax
cmp    $0x3,%cl
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
mov    $0x2,%r14d
cmp    %dil,0xa(%rbp)
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
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
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    %rdx,%r10
cmpb   $0x0,0xf(%r15)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1285>
mov    $0x1,%eax
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
mov    0x8(%r12),%rcx
lea    -0x1e6099(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r13,8),%rdx
mov    %rdx,%rsi
and    %rcx,%rsi
dec    %rsi
xor    %rcx,%rsi
and    %rdx,%rsi
xor    %rcx,%rsi
mov    %rsi,0x8(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
movzwl 0x60(%r13,%rbx,2),%ebp
cmp    $0x1,%rbp
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16e3>
movq   $0x0,0x20(%r13,%rbx,8)
mov    0x8(%rsp),%r12
addq   $0xfffffffffffffff0,0x200(%r12)
decq   0x208(%r12)
mov    0x80(%r12),%rax
mov    %rax,(%rsi)
mov    %rsi,0x80(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1f08>
lea    0x3(%rbp),%r13d
and    $0x3c,%r13d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1168>
lea    0x3(%rbp),%r13
and    $0xfffffffffffffffc,%r13
mov    %rbp,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
cmp    %rax,%r13
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1168>
lea    (%rdx,%r14,8),%rdi
mov    %r12,0x30(%rsp)
lea    (%rdx,%rax,8),%r12
lea    0x1(%r14),%rbx
lea    (%rdx,%r14,8),%rsi
add    $0x8,%rsi
mov    %rbp,%r15
sub    %r14,%r15
lea    0x0(,%r15,8),%rdx
mov    %r10,0x10(%rsp)
mov    %r8,%r13
call   *0x18b35e(%rip)        # ADDR <memmove@GLIBC_2.2.5>
imul   %r13,%rbx
add    %r12,%rbx
imul   %r13,%r14
add    %r12,%r14
imul   %r13,%r15
mov    %r14,%rdi
mov    %rbx,%rsi
mov    %r15,%rdx
call   *0x18b33d(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x10(%rsp),%r10
dec    %rbp
mov    0x30(%rsp),%rcx
mov    0x8(%rcx),%rax
xor    %rax,%rbp
and    0x38(%rsp),%rbp
xor    %rax,%rbp
mov    %rbp,0x8(%rcx)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c53>
lea    0x8(%r8),%rdx
imul   %rdx,%r13
lea    0xf(%r13),%rbx
and    $0xfffffffffffffff0,%rbx
cmp    $0x177,%r13
mov    %r12,0x30(%rsp)
mov    %r10,0x10(%rsp)
mov    %rdx,0x48(%rsp)
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1516>
lea    -0x1e40b3(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl 0x0(%r13,%rax,1),%r15d
cmp    $0xff,%r15
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2979>
add    %rbx,0x200(%rcx)
incq   0x208(%rcx)
incq   0x218(%rcx)
cmp    $0x3d,%r15b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x252f>
mov    0x10(%rcx,%r15,8),%r12
test   %r12,%r12
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x11e5>
mov    (%r12),%rax
mov    %rax,0x10(%rcx,%r15,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1272>
cmp    $0x101,%r13
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1535>
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23eb>
mov    %rax,%r12
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%r12)
mov    %r15,0x8(%r12)
mov    %r12,0x220(%rdi)
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x255b>
cmp    $0x7e0,%rbx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x126e>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %bx
movzwl %ax,%eax
dec    %rax
mov    %rax,%rcx
imul   %rbx,%rcx
lea    (%r12,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x10(%rdi,%r15,8),%rsi
mov    %rsi,0x40(%r12,%rcx,1)
mov    %rdx,0x10(%rdi,%r15,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1247>
add    $0x40,%r12
mov    %r12,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0x18b220(%rip)        # ADDR <memset@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x154e>
mov    %r10,%r15
lea    0x10(%rbp),%rsi
movzbl 0x4(%rbp),%ecx
mov    $0x7,%edx
lea    0x8(%rbp),%rdi
mov    %r14,%r8
call   ADDR <expanse_trie::mutate::linear_remove_slot>
movzbl 0x4(%rbp),%eax
mov    %eax,%edx
dec    %dl
mov    %dl,0x4(%rbp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x162f>
add    $0xf6,%al
cmp    $0xf8,%al
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2568>
lea    0x8(%rbp),%rax
movzbl (%rax),%ecx
and    $0xf,%cl
mov    $0x1,%eax
shl    %cl,%eax
cmp    $0x1,%dl
mov    0x8(%rsp),%rdi
mov    %r15,%r10
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1639>
movzbl 0x9(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x2,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1639>
movzbl 0xa(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x3,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1639>
movzbl 0xb(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x4,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1639>
movzbl 0xc(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x5,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1639>
movzbl 0xd(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x6,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1639>
movzbl 0xe(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
cmp    $0x7,%dl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1631>
movzbl 0xf(%rbp),%ecx
and    $0xf,%cl
bts    %ecx,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1631>
mov    0x8(%r13),%ecx
movzbl 0xc(%r13),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x140f>
movzwl 0x8(%r13),%ecx
movzbl 0xa(%r13),%eax
shl    $0x10,%eax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1434>
mov    0x0(%rbp),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1434>
xor    %eax,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1434>
lea    0x0(,%rax,8),%rdx
lea    0x8(%rdx),%r14d
and    $0x1ff0,%r14d
lea    -0x1e42c3(%rip),%rsi        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rsi,%rax,8),%ebx
cmp    $0xff,%rbx
mov    %r9,0x38(%rsp)
mov    %rcx,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29f6>
add    %r14,0x200(%rdi)
incq   0x208(%rdi)
incq   0x218(%rdi)
cmp    $0x3d,%bl
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x257f>
mov    0x10(%rdi,%rbx,8),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ca0>
mov    0x0(%rbp),%rax
mov    %rax,0x10(%rdi,%rbx,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1d21>
movzwl 0x0(%rbp),%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1434>
mov    0x8(%r13),%ecx
movzwl 0xc(%r13),%eax
shl    $0x20,%rax
or     %rcx,%rax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1434>
mov    0x8(%r13),%ecx
movzwl 0xc(%r13),%esi
movzbl 0xe(%r13),%eax
shl    $0x20,%rsi
or     %rcx,%rsi
shl    $0x30,%rax
or     %rsi,%rax
cmp    %rdx,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x547>
mov    0x0(%r13),%r10
pxor   %xmm0,%xmm0
movdqu %xmm0,0x0(%r13)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c53>
mov    0x8(%rsp),%rdi
call   *0x18b275(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %rbx,%rdi
call   *0x18af64(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x9c8>
mov    $0x18,%eax
cmp    $0x19,%r11d
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1480>
xor    %eax,%eax
cmp    $0x21,%r11d
setb   %al
shl    $0x5,%eax
mov    %rsi,%rcx
cmp    $0x3,%esi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x14a5>
cmp    $0x11,%esi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1495>
lea    0x3(%rsi),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x14a5>
xor    %ecx,%ecx
cmp    $0x19,%esi
setae  %cl
lea    0x18(,%rcx,8),%rcx
lea    (%r8,%r9,8),%r13
cmp    %ecx,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x14db>
lea    0x8(%r13),%rsi
sub    %r9,%r11
shl    $0x3,%r11
mov    %rdi,%r15
mov    %r13,%rdi
mov    %rdx,%r14
mov    %r11,%rdx
mov    %r10,%r13
call   *0x18afb3(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r13,%r10
mov    %r15,%rdi
mov    %r14,%rdx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x190b>
mov    %r11,%rcx
cmp    $0x3,%r11d
mov    %r8,(%rsp)
mov    %r9,0x38(%rsp)
mov    %r13,0x40(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1502>
cmp    $0x11,%r11d
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x170d>
lea    0x2(%rsi),%ecx
and    $0x3c,%ecx
shl    $0x3,%rcx
lea    0x8(%rcx),%r13d
and    $0x3f0,%r13d
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x172a>
mov    %rcx,%rdi
mov    %rbx,%rsi
call   *0x18b176(%rip)        # ADDR <_DYNAMIC+0x500>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r13
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1f46>
mov    $0x10,%esi
mov    %r13,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%r12
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23fb>
lea    0x0(,%r14,8),%rdx
mov    0x18ae53(%rip),%r15        # ADDR <memcpy@GLIBC_2.14>
mov    %r12,%rdi
mov    (%rsp),%rbx
mov    %rbx,%rsi
call   *%r15
lea    0x1(%r14),%rax
mov    %rax,0x40(%rsp)
lea    (%rbx,%r14,8),%rsi
add    $0x8,%rsi
lea    (%r12,%r14,8),%rdi
mov    %rbp,%rax
sub    %r14,%rax
mov    %rax,0x60(%rsp)
lea    0x0(,%rax,8),%rdx
call   *%r15
mov    $0x1,%ecx
mov    $0x2,%eax
lea    0x1(%rbp),%rdx
cmp    $0x3,%rdx
mov    0x20(%rsp),%rbx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b42>
cmp    $0x11,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15cd>
cmp    $0x3,%rdx
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15e8>
mov    $0x2,%ecx
mov    $0x4,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b42>
cmp    $0x19,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15f9>
mov    $0x18,%eax
mov    $0x18,%ecx
cmp    $0x11,%rdx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x15ee>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b42>
lea    0x4(%rbp),%eax
and    $0x3c,%eax
lea    0x3(%rbp),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b42>
cmp    $0x21,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x161a>
xor    %eax,%eax
cmp    $0x19,%rdx
setne  %al
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b42>
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b2f>
mov    $0x20,%ecx
mov    $0x24,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1b42>
xor    %eax,%eax
mov    0x8(%rsp),%rdi
mov    %r15,%r10
mov    %ax,0x6(%rbp)
mov    (%r12),%rax
movzbl 0x4(%rax),%ecx
test   %cl,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x16a2>
cmp    $0x7,%r13b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1676>
mov    0x8(%r12),%rax
lea    -0x1e6693(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r13,8),%rdx
mov    %rdx,%rsi
and    %rax,%rsi
dec    %rsi
xor    %rax,%rsi
and    %rdx,%rsi
xor    %rax,%rsi
mov    %rsi,0x8(%r12)
cmp    $0x1,%bl
sete   %al
cmp    $0x3,%cl
setae  %cl
or     %al,%cl
mov    $0x1,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
mov    %r12,%rsi
mov    %r10,%rbx
call   ADDR <expanse_trie::mutate::downgrade_l7_to_l3::<false>>
mov    %rbx,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c53>
xor    %ecx,%ecx
xor    %edx,%edx
cmp    $0x1,%bl
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
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c53>
lea    -0x1(%rbp),%rdx
mov    %rdx,%rax
cmp    $0x3,%rdx
mov    0x8(%rsp),%r8
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1eb2>
cmp    $0x11,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1e98>
lea    0x2(%rbp),%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1eb2>
xor    %eax,%eax
cmp    $0x21,%r11
setb   %al
shl    $0x8,%eax
cmp    $0x19,%r11
mov    $0xc0,%r13d
cmovae %rax,%r13
mov    %r13,%rcx
lea    -0x1e4649(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rcx,%rax,1),%r15d
cmp    $0xff,%r15
mov    %rdx,0x60(%rsp)
mov    %r12,0x30(%rsp)
mov    %rdi,0x8(%rsp)
mov    %r10,0x10(%rsp)
mov    %rsi,0x20(%rsp)
mov    %r11,0x48(%rsp)
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x299b>
add    %r13,0x200(%rdi)
incq   0x208(%rdi)
incq   0x218(%rdi)
cmp    $0x3d,%r15b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x252f>
mov    0x10(%rdi,%r15,8),%rbp
test   %rbp,%rbp
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1798>
mov    0x0(%rbp),%rax
mov    %rax,0x10(%rdi,%r15,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1819>
mov    %rcx,0x68(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23eb>
mov    %rax,%rbp
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,0x0(%rbp)
mov    %r15,0x8(%rbp)
mov    %rbp,0x220(%rdi)
test   %r13,%r13
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x255b>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r13w
movzwl %ax,%eax
dec    %rax
mov    %rax,%rcx
imul   %r13,%rcx
lea    (%rcx,%rbp,1),%rdx
add    $0x40,%rdx
mov    0x10(%rdi,%r15,8),%rsi
mov    %rsi,0x40(%rbp,%rcx,1)
mov    %rdx,0x10(%rdi,%r15,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x17e9>
add    $0x40,%rbp
mov    0x68(%rsp),%rcx
mov    %rbp,%rdi
xor    %esi,%esi
mov    %rcx,%rdx
call   *0x18ac79(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    (%rsp),%rsi
mov    0x38(%rsp),%r13
lea    0x0(,%r13,8),%r15d
mov    0x18ab71(%rip),%r12        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,%rdi
mov    %r15,%rdx
call   *%r12
add    %rbp,%r15
mov    0x40(%rsp),%rsi
add    $0x8,%rsi
mov    0x48(%rsp),%rdx
sub    %r13,%rdx
shl    $0x3,%rdx
mov    %r15,%rdi
call   *%r12
mov    %rbp,0x20(%rbx,%r14,8)
mov    0x20(%rsp),%rdi
cmp    $0x3,%edi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x188b>
mov    0x60(%rsp),%rdx
mov    0x30(%rsp),%r12
mov    0x8(%rsp),%rcx
mov    0x10(%rsp),%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18bc>
cmp    $0x11,%edi
mov    0x60(%rsp),%rdx
mov    0x30(%rsp),%r12
mov    0x8(%rsp),%rcx
mov    0x10(%rsp),%r10
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18ac>
add    $0x3,%edi
and    $0x3c,%edi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x18bc>
xor    %eax,%eax
cmp    $0x19,%edi
setae  %al
lea    0x18(,%rax,8),%rdi
lea    -0x1e47db(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
shl    $0x3,%edi
lea    0x8(%rdi),%rsi
and    $0xfffffffffffffff0,%rsi
movzbl (%rdi,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x29d5>
sub    %rsi,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3d,%dil
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2945>
mov    0x10(%rcx,%rdi,8),%rax
mov    (%rsp),%rsi
mov    %rax,(%rsi)
mov    %rsi,0x10(%rcx,%rdi,8)
mov    %rcx,%rdi
mov    %edx,%eax
shr    $0x3,%eax
mov    $0xfffffffffffffffe,%rsi
mov    %edx,%ecx
rol    %cl,%rsi
and    $0x18,%eax
and    %rsi,(%rbx,%rax,1)
mov    0x8(%r12),%rax
movzbl %al,%ecx
cmp    $0x15,%rcx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1971>
movzwl 0xd(%r12),%eax
mov    %ax,0x7c(%rsp)
mov    0x9(%r12),%eax
mov    %eax,0x78(%rsp)
mov    0x18(%rsp),%rax
cmp    $0x2,%al
mov    %r10,0x10(%rsp)
mov    %rdi,%r15
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1987>
movzbl %al,%edx
mov    %r12,%rdi
mov    $0x1,%esi
call   *0x18adc9(%rip)        # ADDR <_DYNAMIC+0x598>
shl    $0x8,%rax
mov    %rax,(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x198f>
dec    %rcx
and    $0xffffffffffffff00,%rax
or     %rcx,%rax
mov    %rax,0x8(%r12)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c53>
movq   $0x0,(%rsp)
movq   $0x0,0x290(%rsp)
mov    (%rbx),%rax
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19a7>
xor    %ecx,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19cc>
mov    0x8(%rbx),%rax
mov    $0x40,%cl
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19cc>
mov    0x10(%rbx),%rax
mov    $0x80,%cl
test   %rax,%rax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19cc>
mov    0x18(%rbx),%rax
mov    $0xc0,%cl
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1aeb>
tzcnt  %rax,%rax
or     %cl,%al
lea    0x98(%rsp),%rsi
xor    %edx,%edx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1a01>
nop
shl    $0x6,%dil
tzcnt  %r9,%rax
or     %dil,%al
add    $0x10,%rsi
lea    0x1(%rdx),%rdi
cmp    $0x1f,%rdx
mov    %rdi,%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24f6>
movzbl %al,%ecx
mov    %ecx,%edi
shr    $0x5,%dil
mov    $0xffffffff,%r8d
shl    %cl,%r8d
not    %r8d
movzbl %dil,%edi
and    (%rbx,%rdi,4),%r8d
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
mov    0x20(%rbx,%rdi,8),%rdi
shr    $0x18,%r8d
mov    (%rdi,%r8,8),%rdi
mov    %rcx,-0x8(%rsi)
mov    %rdi,(%rsi)
cmp    $0xff,%cl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ae0>
inc    %al
mov    %eax,%edi
shr    $0x6,%dil
movzbl %dil,%r8d
mov    (%rbx,%r8,8),%r9
mov    %eax,%ecx
shr    %cl,%r9
shl    %cl,%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19e0>
lea    0x1(%r8),%rdi
cmp    $0x4,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ae0>
mov    0x8(%rbx,%r8,8),%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19e0>
lea    0x2(%r8),%rdi
cmp    $0x4,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ae0>
mov    0x10(%rbx,%r8,8),%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19e0>
lea    0x3(%r8),%rdi
cmp    $0x4,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ae0>
mov    0x18(%rbx,%r8,8),%r9
test   %r9,%r9
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x19e0>
inc    %rdx
mov    %rdx,0x290(%rsp)
lea    0x9(%r12),%rax
mov    %rax,0x20(%rsp)
movabs $0x7fffffff0,%rbp
lea    0x2a0(%rsp),%rdi
lea    0x90(%rsp),%rsi
mov    $0x208,%edx
call   *0x18a896(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    $0x4,%r13d
lea    -0x1e4a3f(%rip),%r14        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
mov    %r15,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25dd>
mov    %rbp,%rax
and    $0xfffffffffffffffc,%rax
add    $0x4,%rax
lea    0x3(%rbp),%rcx
and    $0xfffffffffffffffc,%rcx
mov    (%rsp),%rdx
lea    (%rdx,%rax,8),%r15
lea    (%r12,%rcx,8),%r13
imul   %rbx,%r14
mov    %r13,%rdi
mov    %r15,%rsi
mov    %r14,%rdx
mov    0x18a84e(%rip),%rax        # ADDR <memcpy@GLIBC_2.14>
mov    %rbp,0x68(%rsp)
mov    %rax,%rbp
call   *%rax
mov    0x40(%rsp),%rsi
imul   %rbx,%rsi
add    %r15,%rsi
add    %r13,%r14
mov    0x60(%rsp),%rdx
imul   %rbx,%rdx
mov    %r14,%rdi
call   *%rbp
mov    0x68(%rsp),%rdi
lea    0x1(%rdi),%rsi
mov    0x30(%rsp),%rdx
mov    0x18(%rsp),%eax
mov    %al,0xf(%rdx)
lea    -0x1(%rdi),%rax
mov    0x8(%rdx),%rcx
xor    %rcx,%rax
and    0x38(%rsp),%rax
xor    %rcx,%rax
mov    %r12,(%rdx)
mov    %rax,0x8(%rdx)
cmp    $0x3,%rsi
mov    0x8(%rsp),%rcx
mov    0x10(%rsp),%r10
mov    0x48(%rsp),%rax
mov    $0x2,%r14d
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1bfc>
cmp    $0x11,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1be4>
add    $0x4,%edi
and    $0x3c,%edi
mov    %rdi,%r14
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1bfc>
mov    $0x18,%r14d
cmp    $0x19,%rsi
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1bfc>
mov    $0x20,%r14d
cmp    $0x21,%rsi
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c5d>
imul   %rax,%r14
lea    0xf(%r14),%rsi
and    $0xfffffffffffffff0,%rsi
cmp    $0x177,%r14
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c6a>
lea    -0x1e4b30(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%r14,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x298a>
sub    %rsi,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
mov    (%rsp),%rdx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2945>
mov    0x10(%rcx,%rdi,8),%rax
mov    %rax,(%rdx)
mov    %rdx,0x10(%rcx,%rdi,8)
mov    $0x1,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
and    $0xfffffffffffffffc,%rdi
add    $0x4,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1bdf>
mov    %r10,%rbx
mov    %rcx,%rdi
call   *0x18aa5a(%rip)        # ADDR <_DYNAMIC+0x538>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r14
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1f46>
mov    (%rsp),%rdi
call   *0x18a735(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    $0x1,%eax
mov    %rbx,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
mov    %rdx,0x40(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23eb>
mov    %rax,%rbp
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,0x0(%rbp)
mov    %rbx,0x8(%rbp)
mov    %rbp,0x220(%rdi)
test   %r14,%r14
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x255b>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r14w
movzwl %ax,%eax
mov    0x40(%rsp),%rdx
dec    %rax
mov    %rax,%rcx
imul   %r14,%rcx
lea    (%rcx,%rbp,1),%r8
add    $0x40,%r8
mov    0x10(%rdi,%rbx,8),%rsi
mov    %rsi,0x40(%rbp,%rcx,1)
mov    %r8,0x10(%rdi,%rbx,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1cf6>
add    $0x40,%rbp
mov    %rbp,%rdi
xor    %esi,%esi
call   *0x18a774(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x18(%rsp),%r14d
mov    (%rsp),%r8
test   %r8,%r8
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1d54>
lea    0x0(,%r8,8),%rdx
mov    %rbp,%rdi
mov    0x38(%rsp),%rsi
call   *0x18a660(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    (%rsp),%r8
lea    0x1(%r8),%rbx
cmp    %r12,%rbx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1d87>
mov    0x38(%rsp),%rax
lea    (%rax,%rbx,8),%rsi
lea    0x0(,%r8,8),%rdi
add    %rbp,%rdi
lea    -0x1(%r12),%rdx
sub    %r8,%rdx
shl    $0x3,%rdx
call   *0x18a62d(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    (%rsp),%r8
lea    0x8(%r13),%rcx
mov    (%rcx),%eax
mov    %rcx,(%rsp)
mov    0x3(%rcx),%ecx
mov    %ecx,0x5b(%rsp)
mov    %eax,0x58(%rsp)
imul   %r15,%r12
cmp    $0x8,%r12
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2430>
imul   %r15,%rbx
mov    %r12,%rdx
sub    %rbx,%rdx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2459>
imul   %r15,%r8
mov    %rdx,%rax
xor    $0x7,%rax
cmp    %rax,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x248a>
lea    (%rsp,%rbx,1),%rsi
add    $0x58,%rsi
lea    (%rsp,%r8,1),%rdi
add    $0x58,%rdi
call   *0x18a69c(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    0x20(%rsp),%rbx
imul   %rbx,%r15
cmp    $0x7,%r15
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x24bd>
lea    (%rsp,%r15,1),%rdi
add    $0x58,%rdi
xor    $0x7,%r15
xor    %esi,%esi
mov    %r15,%rdx
call   *0x18a692(%rip)        # ADDR <memset@GLIBC_2.2.5>
test   %rbx,%rbx
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23d2>
mov    %rbp,0x0(%r13)
mov    0x58(%rsp),%eax
mov    0x5b(%rsp),%ecx
mov    (%rsp),%rdx
mov    %ecx,0x3(%rdx)
mov    %eax,(%rdx)
and    $0x70,%r14b
dec    %bl
or     %r14b,%bl
mov    %bl,0xf(%r13)
mov    0x48(%rsp),%rcx
lea    0x0(,%rcx,8),%eax
lea    0x8(,%rcx,8),%esi
and    $0xfffffff0,%esi
lea    -0x1e4d6e(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rax,%rcx,1),%edi
cmp    $0xff,%rdi
mov    0x8(%rsp),%rcx
mov    0x10(%rsp),%r10
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a31>
sub    %rsi,0x200(%rcx)
decq   0x208(%rcx)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2945>
mov    0x10(%rcx,%rdi,8),%rax
mov    0x38(%rsp),%rdx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c4b>
mov    $0x18,%eax
cmp    $0x19,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1eb2>
mov    $0x20,%eax
cmp    $0x21,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1eb2>
lea    0x2(%rbp),%eax
and    $0xfffffffc,%eax
mov    %rbp,%rcx
cmp    $0x3,%bp
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ee5>
cmp    $0x11,%bp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ec9>
lea    0x3(%rbp),%ecx
and    $0x3c,%ecx
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ee5>
mov    $0x18,%ecx
cmp    $0x19,%bp
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ee5>
mov    $0x20,%ecx
cmp    $0x21,%bp
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1ee5>
lea    0x3(%rbp),%ecx
and    $0xfffffffc,%ecx
add    %rsi,%rdi
cmp    %ecx,%eax
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1f12>
lea    0x10(%rdi),%rsi
sub    %r12,%rdx
shl    $0x4,%rdx
mov    %r10,%r12
call   *0x18a580(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r12,%r10
mov    0x8(%rsp),%r12
mov    0x30(%rsp),%r8
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x21c9>
mov    %rdx,%rcx
cmp    $0x3,%edx
mov    %rbp,0x18(%rsp)
mov    %r10,0x10(%rsp)
mov    %rsi,(%rsp)
mov    %rdi,0x38(%rsp)
mov    %rdx,0x48(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1f3d>
cmp    $0x11,%edx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1f6b>
lea    0x2(%rbp),%ecx
and    $0x3c,%ecx
mov    %rcx,%r9
shl    $0x4,%r9
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1f94>
lea    -0x1e7018(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0x18781c(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x1809a5(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x2f(%rsp),%rdx
mov    $0x11,%esi
call   *0x18a4f5(%rip)        # ADDR <_DYNAMIC+0x2c8>
cmp    $0x19,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x20d3>
cmp    $0x21,%edx
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x20da>
lea    0x2(%rbp),%ecx
and    $0xfffffffc,%ecx
mov    %ecx,%r9d
shl    $0x4,%r9d
cmp    $0x15,%bp
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x20e1>
lea    -0x1e4eb3(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%r9,%rax,1),%ebp
cmp    $0xff,%rbp
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x20e1>
add    %r9,0x200(%r8)
incq   0x208(%r8)
incq   0x218(%r8)
cmp    $0x3e,%bpl
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2594>
mov    0x10(%r8,%rbp,8),%r11
test   %r11,%r11
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1fe3>
mov    (%r11),%rax
mov    %rax,0x10(%r8,%rbp,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2071>
cmp    $0x11,%ecx
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x20f5>
mov    %rcx,0x40(%rsp)
mov    %r9,0x20(%rsp)
mov    $0x1000,%edi
mov    $0x40,%esi
call   ADDR <__rustc::__rust_alloc_zeroed>
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x23eb>
mov    %rax,%r11
mov    0x8(%rsp),%rdi
mov    0x220(%rdi),%rax
mov    %rax,(%r11)
mov    %rbp,0x8(%r11)
mov    %r11,0x220(%rdi)
cmpq   $0x0,0x40(%rsp)
mov    0x20(%rsp),%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x255b>
mov    $0xfc0,%eax
xor    %edx,%edx
div    %r9d
dec    %rax
mov    %rax,%rcx
imul   %r9,%rcx
lea    (%r11,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x10(%rdi,%rbp,8),%rsi
mov    %rsi,0x40(%r11,%rcx,1)
mov    %rdx,0x10(%rdi,%rbp,8)
cmp    $0x1,%rax
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2046>
add    $0x40,%r11
mov    %r11,%rdi
xor    %esi,%esi
mov    %r9,%rdx
mov    %r11,%rbp
call   *0x18a41e(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %rbp,%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2111>
lea    0x180ae7(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4a8>
mov    $0x8,%edx
xor    %edi,%edi
mov    %rbp,%rsi
call   *0x18a347(%rip)        # ADDR <_DYNAMIC+0x250>
lea    -0x1e709a(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8db>
lea    0x180b41(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x520>
mov    $0xa1,%esi
call   *0x18a356(%rip)        # ADDR <_DYNAMIC+0x278>
lea    -0x20a0d9(%rip),%rdi        # ADDR <core::unicode::unicode_data::cf::SHORT_OFFSET_RUNS+0xf8>
lea    0x180a98(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x490>
mov    $0x4,%esi
call   *0x18a495(%rip)        # ADDR <_DYNAMIC+0x3d0>
mov    $0x180,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x20e4>
mov    $0x200,%ebp
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x20e4>
mov    %r9,%rbp
mov    0x8(%rsp),%rdi
mov    %rbp,%rsi
call   *0x18a5a6(%rip)        # ADDR <_DYNAMIC+0x500>
mov    %rbp,%r9
mov    $0x10,%esi
mov    %r9,%rdi
mov    %r9,%rbp
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rdi
test   %rax,%rax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2508>
mov    %rdi,0x20(%rsp)
mov    %r12d,%ebp
shl    $0x4,%ebp
mov    (%rsp),%rsi
mov    %rbp,%rdx
call   *0x18a287(%rip)        # ADDR <memcpy@GLIBC_2.14>
add    0x20(%rsp),%rbp
mov    0x38(%rsp),%rsi
add    $0x10,%rsi
mov    0x48(%rsp),%rdx
sub    %r12,%rdx
shl    $0x4,%rdx
mov    %rbp,%rdi
call   *0x18a264(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x20(%rsp),%rax
mov    %rax,0x20(%r13,%rbx,8)
mov    0x18(%rsp),%rbp
mov    %rbp,%rsi
cmp    $0x3,%bp
mov    0x30(%rsp),%r8
mov    0x8(%rsp),%r12
mov    0x10(%rsp),%r10
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2183>
cmp    $0x11,%bp
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2314>
lea    0x3(%rbp),%esi
and    $0x3c,%esi
shl    $0x4,%esi
lea    -0x1e50a5(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rsi,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2a47>
sub    %rsi,0x200(%r12)
decq   0x208(%r12)
cmp    $0x3e,%dil
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2945>
mov    0x10(%r12,%rdi,8),%rax
mov    (%rsp),%rcx
mov    %rax,(%rcx)
mov    %rcx,0x10(%r12,%rdi,8)
dec    %ebp
mov    %bp,0x60(%r13,%rbx,2)
mov    %r15d,%eax
shr    $0x3,%eax
mov    $0xfffffffffffffffe,%rdx
mov    %r15d,%ecx
rol    %cl,%rdx
and    $0x18,%eax
and    %rdx,0x0(%r13,%rax,1)
movdqu 0x0(%r13),%xmm0
movdqu 0x10(%r13),%xmm1
movdqa %xmm1,%xmm2
psrlw  $0x1,%xmm2
movdqa -0x20bb99(%rip),%xmm3        # ADDR <anon.ADDR.407.llvm.ADDR+0x10>
pand   %xmm3,%xmm2
psubb  %xmm2,%xmm1
movdqa -0x20b8f9(%rip),%xmm2        # ADDR <anon.ADDR.407.llvm.ADDR+0x2c0>
movdqa %xmm1,%xmm4
pand   %xmm2,%xmm4
psrlw  $0x2,%xmm1
pand   %xmm2,%xmm1
paddb  %xmm4,%xmm1
movdqa %xmm1,%xmm4
psrlw  $0x4,%xmm4
paddb  %xmm1,%xmm4
movdqa -0x20bc73(%rip),%xmm1        # ADDR <anon.ADDR.283.llvm.ADDR+0xf6>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x22eb>
cmp    $0x7,%r14b
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x22c7>
mov    0x8(%r8),%rax
lea    -0x1e72e5(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%rdx,%r14,8),%rdx
mov    %rdx,%rsi
and    %rax,%rsi
dec    %rsi
xor    %rax,%rsi
and    %rdx,%rsi
xor    %rax,%rsi
mov    %rsi,0x8(%r8)
mov    $0x1,%eax
cmp    $0x6,%ecx
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x549>
mov    %r12,%rdi
mov    %r8,%rsi
mov    %r10,%rbx
call   ADDR <expanse_trie::mutate::downgrade_b_to_l7::<false>>
mov    %rbx,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c53>
addq   $0xffffffffffffff80,0x200(%r12)
decq   0x208(%r12)
mov    0x28(%r12),%rax
mov    %rax,0x0(%r13)
mov    %r13,0x28(%r12)
movdqu %xmm0,(%r8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c53>
mov    %rbp,%rax
mov    %r10,%rbp
mov    $0x180,%esi
cmp    $0x19,%ax
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2340>
mov    $0x200,%esi
cmpw   $0x21,0x18(%rsp)
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2340>
mov    0x18(%rsp),%rsi
shl    $0x4,%esi
add    $0x30,%esi
and    $0xffffffc0,%esi
mov    0x8(%rsp),%r12
mov    %r12,%rdi
call   *0x18a382(%rip)        # ADDR <_DYNAMIC+0x538>
mov    (%rsp),%rdi
call   *0x18a070(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x30(%rsp),%r8
mov    %rbp,%r10
mov    0x18(%rsp),%rbp
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x21c9>
mov    %rcx,%rdi
mov    %r10,%rbx
mov    %rdx,%r14
call   *0x18a357(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %r14,%rdi
call   *0x18a046(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%r10
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c53>
lea    0x1808a7(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x568>
mov    $0x7,%edx
xor    %edi,%edi
mov    %r15,%rsi
call   *0x18a047(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x180878(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x550>
mov    $0x7,%edx
mov    %rsi,%rdi
mov    %rax,%rsi
call   *0x18a02f(%rip)        # ADDR <_DYNAMIC+0x250>
lea    -0x1e7362(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x1808e9(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5e0>
mov    $0x12,%esi
call   *0x18a196(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    -0x1e737b(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc92b>
lea    0x180888(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x598>
mov    $0x12,%esi
call   *0x18a17d(%rip)        # ADDR <_DYNAMIC+0x3d0>
mov    $0x40,%edi
mov    $0x1000,%esi
call   *0x18a01d(%rip)        # ADDR <_DYNAMIC+0x280>
mov    $0x10,%edi
mov    %r13,%rsi
call   *0x18a00f(%rip)        # ADDR <_DYNAMIC+0x280>
lea    0x1807f8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x538>
mov    %r15,%rdi
mov    %rcx,%rsi
call   *0x18a1bc(%rip)        # ADDR <_DYNAMIC+0x440>
lea    0x1881e5(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f38>
mov    $0x7,%edx
xor    %edi,%edi
call   *0x189fb8(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1881d1(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f38>
mov    $0x7,%edx
xor    %edi,%edi
mov    %r12,%rsi
call   *0x189fa1(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1881d2(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f50>
mov    $0x7,%edx
call   *0x189f8f(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1881c0(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7f50>
mov    $0x7,%edx
mov    %rbx,%rdi
mov    %r12,%rsi
call   *0x189f77(%rip)        # ADDR <_DYNAMIC+0x250>
lea    -0x1e7558(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc7ed>
lea    0x180819(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5c8>
mov    $0x2b,%esi
call   *0x189f86(%rip)        # ADDR <_DYNAMIC+0x278>
lea    -0x1e7571(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc7ed>
lea    0x1807b8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x580>
mov    $0x2b,%esi
call   *0x189f6d(%rip)        # ADDR <_DYNAMIC+0x278>
lea    0x18081e(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5f8>
mov    $0x7,%esi
mov    $0x7,%edx
mov    %r15,%rdi
call   *0x189f2b(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x1807bc(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x5b0>
mov    $0x7,%esi
mov    $0x7,%edx
mov    %r15,%rdi
call   *0x189f11(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x17f3d2(%rip),%rdx        # ADDR <anon.ADDR.888.llvm.ADDR>
call   *0x189ebc(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1805e5(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x400>
mov    $0x8,%esi
call   *0x189eaa(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1806c3(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4f0>
mov    $0x20,%esi
call   *0x189e98(%rip)        # ADDR <_DYNAMIC+0x208>
mov    $0x10,%edi
mov    %rbp,%rsi
call   *0x189f02(%rip)        # ADDR <_DYNAMIC+0x280>
lea    -0x1e7567(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc883>
lea    0x180594(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x3e8>
mov    $0xe,%esi
call   *0x18a039(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x1803e2(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %r15,%rdi
call   *0x189e5c(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x180645(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4c0>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0x189e45(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1803b6(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
call   *0x18a048(%rip)        # ADDR <_DYNAMIC+0x418>
lea    0x180591(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x430>
mov    $0x8,%edi
mov    $0x8,%esi
call   *0x189e21(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x180392(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %rbx,%rdi
call   *0x189e0c(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x18037d(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    $0x3e,%esi
mov    %rbp,%rdi
call   *0x189df7(%rip)        # ADDR <_DYNAMIC+0x208>
ud2
mov    %r10,%rdi
mov    %r10,0x8(%rsp)
call   *0x18a117(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %r15,%rdi
call   *0x189e06(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x8(%rsp),%r10
nopw   0x0(%rax,%rax,1)
inc    %r13
cmp    $0xc,%r13
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2695>
mov    -0x10(%rbx,%r13,4),%eax
test   %eax,%eax
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25d0>
mov    (%rbx,%r13,8),%r15
test   %r15,%r15
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2912>
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
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2649>
cmp    $0x11,%eax
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2639>
add    $0x3,%eax
and    $0x3c,%eax
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2649>
xor    %ecx,%ecx
cmp    $0x19,%eax
setae  %cl
lea    0x18(,%rcx,8),%rax
lea    0x0(,%rax,8),%ecx
lea    0x8(,%rax,8),%esi
and    %ebp,%esi
movzbl (%rcx,%r14,1),%edi
cmp    $0xff,%rdi
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25ab>
sub    %rsi,0x200(%r10)
decq   0x208(%r10)
cmp    $0x3d,%dil
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2945>
mov    0x10(%r10,%rdi,8),%rax
mov    %rax,(%r15)
mov    %r15,0x10(%r10,%rdi,8)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x25d0>
addq   $0xffffffffffffff80,0x200(%r10)
decq   0x208(%r10)
mov    0x28(%r10),%rax
mov    %rax,(%rbx)
mov    %rbx,0x28(%r10)
mov    0x18(%rsp),%rbx
test   %bl,%bl
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2957>
mov    0x4a0(%rsp),%r9
mov    $0x7,%al
movzbl %al,%eax
div    %bl
movzbl %al,%eax
cmp    %rax,%r9
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28b0>
movq   $0x0,0x290(%rsp)
test   %r9,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28f4>
mov    0x2a0(%rsp),%rax
or     (%rsp),%rax
mov    0x2a8(%rsp),%rcx
mov    %rax,0x90(%rsp)
mov    %rcx,0x98(%rsp)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
cmp    $0x1,%r9
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28f7>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2964>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28f7>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2964>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28f7>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2964>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28f7>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2964>
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
je     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28f7>
cmp    $0x1f,%r8
ja     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2964>
mov    (%rsp),%rcx
or     0x2f0(%rsp),%rcx
mov    0x2f8(%rsp),%rax
shl    $0x4,%r8
mov    %rcx,0x90(%rsp,%r8,1)
mov    %rax,0x98(%rsp,%r8,1)
mov    0x290(%rsp),%r8
inc    %r8
mov    %r8,0x290(%rsp)
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x28f7>
lea    0x2a0(%rsp),%rcx
mov    %r10,%rdi
mov    %r12,%rsi
mov    $0x1,%edx
mov    %r9,%r8
call   ADDR <expanse_trie::mutate_map::build_map_leaf::<false>>
cmp    $0x2,%bl
jb     ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x9c8>
movzbl %bl,%edx
cmp    $0x8,%bl
jae    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x292b>
dec    %rdx
lea    0x78(%rsp),%rsi
mov    0x20(%rsp),%rdi
call   *0x189ac1(%rip)        # ADDR <memcpy@GLIBC_2.14>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x9c8>
xor    %r8d,%r8d
movzbl %bl,%edx
lea    0x90(%rsp),%rcx
mov    %r10,%rdi
mov    %r12,%rsi
call   ADDR <expanse_trie::mutate_map::write_map_immed::<false>>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x9c8>
lea    -0x1e7911(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8d5>
lea    0x1802b8(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x508>
mov    $0x6,%esi
call   *0x189c3d(%rip)        # ADDR <_DYNAMIC+0x3d0>
lea    0x17efde(%rip),%rcx        # ADDR <anon.ADDR.892.llvm.ADDR>
mov    $0x1,%edi
mov    %rdx,%rsi
mov    $0x7,%edx
call   *0x189aa3(%rip)        # ADDR <_DYNAMIC+0x250>
lea    0x17ff9c(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    $0x3e,%esi
call   *0x189a49(%rip)        # ADDR <_DYNAMIC+0x208>
lea    0x1801ea(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x478>
call   *0x189c4c(%rip)        # ADDR <_DYNAMIC+0x418>
lea    0x180255(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x4f0>
mov    $0x20,%esi
mov    %r8,%rdi
call   *0x189a27(%rip)        # ADDR <_DYNAMIC+0x208>
mov    %rcx,%rdi
mov    %rbx,%rsi
call   *0x189d13(%rip)        # ADDR <_DYNAMIC+0x500>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1535>
mov    %r10,%rbx
mov    %rcx,%rdi
call   *0x189d3a(%rip)        # ADDR <_DYNAMIC+0x538>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1c89>
mov    %r13,%rsi
mov    %rcx,%r12
call   *0x189cf1(%rip)        # ADDR <_DYNAMIC+0x500>
mov    $0x10,%esi
mov    %r12,%r15
mov    %r12,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rbp
test   %rax,%rax
mov    (%rsp),%rsi
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x182b>
mov    $0x10,%edi
mov    %r15,%rsi
call   *0x189a43(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %rcx,%rdi
mov    %rdx,%r14
mov    %rcx,%r15
mov    %r10,%r13
call   *0x189ce9(%rip)        # ADDR <_DYNAMIC+0x538>
mov    (%rsp),%rdi
call   *0x1899d7(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x14cd>
mov    %r14,%rsi
mov    %rdx,%r14
call   *0x189c96(%rip)        # ADDR <_DYNAMIC+0x500>
mov    $0x10,%esi
mov    %r14,%rbx
mov    %r14,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%rbp
test   %rax,%rax
mov    0x18(%rsp),%r14d
jne    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x1d31>
mov    $0x10,%edi
mov    %rbx,%rsi
call   *0x1899e7(%rip)        # ADDR <_DYNAMIC+0x280>
mov    %rcx,%rdi
mov    %r10,%rbx
call   *0x189c93(%rip)        # ADDR <_DYNAMIC+0x538>
mov    0x38(%rsp),%rdi
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x237c>
mov    %r10,%rbp
jmp    ADDR <expanse_trie::mutate_map::map_remove::<false, false>+0x2340>
int3
