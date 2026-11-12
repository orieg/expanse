push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x3d8,%rsp
mov    %rdi,%rbx
lea    0xb0(%rsp),%rdi
xor    %r15d,%r15d
mov    $0x1f0,%edx
xor    %esi,%esi
call   *0xa49e4(%rip)        # ADDR <memset@GLIBC_2.2.5>
xorps  %xmm0,%xmm0
movups %xmm0,0x2d0(%rsp)
movups %xmm0,0x2e0(%rsp)
movups %xmm0,0x2f0(%rsp)
movups %xmm0,0x300(%rsp)
movq   $0x0,0x80(%rsp)
movq   $0x0,0x98(%rsp)
movl   $0x3,0xa8(%rsp)
movups %xmm0,0x2a0(%rsp)
movups %xmm0,0x2b0(%rsp)
movups %xmm0,0x2b9(%rsp)
movq   $0xffffffffffffffff,0x310(%rsp)
movups %xmm0,0x318(%rsp)
movups %xmm0,0x328(%rsp)
movups %xmm0,0x332(%rsp)
mov    0x8(%rbx),%r14
mov    %rbx,0x68(%rsp)
mov    0x10(%rbx),%rax
test   %rax,%rax
mov    %r14,0x58(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x197b>
lea    0x2d0(%rsp),%r13
lea    (%r14,%rax,8),%rax
mov    %rax,0x350(%rsp)
lea    0xa1eac(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
mov    %rax,0x28(%rsp)
xor    %r15d,%r15d
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x112>
xchg   %ax,%ax
xor    %ebx,%ebx
add    $0x8,%r14
movzbl %bl,%eax
mov    0x358(%rsp),%r15
add    %rax,%r15
cmp    0x350(%rsp),%r14
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x197b>
mov    %r15,0x358(%rsp)
mov    (%r14),%rax
mov    %rax,0x30(%rsp)
mov    %rax,0x78(%rsp)
lea    0x78(%rsp),%rax
mov    0x78(%rsp),%rbp
cmpl   $0x2,0x80(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x400>
mov    %rbp,%rax
shr    $0x8,%rax
cmp    %rax,0x310(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x810>
mov    0x328(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x4ce>
mov    %ebp,%eax
shr    $0x3,%eax
and    $0x18,%eax
mov    (%rbx,%rax,1),%rcx
bts    %rbp,%rcx
mov    %rcx,(%rbx,%rax,1)
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
incq   0x338(%rsp)
mov    0x340(%rsp),%eax
lea    0x1(%rax),%ecx
mov    %cx,0x340(%rsp)
mov    0x2d0(%rsp),%rcx
mov    %al,0x8(%rcx)
cmpw   $0x100,0x340(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa85>
mov    0x338(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movq   $0x0,0x338(%rsp)
mov    0x320(%rsp),%rax
cmp    $0x2,%rax
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movzbl 0x319(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x21e>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c68bc(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movzbl 0x31a(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x263>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c6901(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movzbl 0x31b(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x2a8>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c6946(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movzbl 0x31c(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x2ed>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c698b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movzbl 0x31d(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x332>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c69d0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movzbl 0x31e(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x377>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c6a15(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movzbl 0x31f(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3b8>
mov    0x308(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c6a56(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a0e>
mov    0xa8(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xee4>
lock subq $0x40,0x2a0(%rsp)
lock decq 0x2a8(%rsp)
mov    0xb8(%rsp),%rax
mov    %rax,(%rbx)
mov    %rbx,0xb8(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2f>
nopl   0x0(%rax)
mov    0xa8(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa90>
cmpq   $0x0,0x80(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x48a>
mov    0x88(%rsp),%rcx
mov    0x90(%rsp),%r15
mov    $0x1,%r12d
test   %rcx,%rcx
mov    %rcx,0x8(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x52c>
mov    %rcx,%rbx
cmp    -0x8(%r15,%rcx,8),%rbp
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6ae>
cmp    $0x1f,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x70a>
mov    $0x2,%edi
cmp    $0x1,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x533>
lea    0x1(%rcx),%rdi
cmp    $0x10,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11a2>
cmp    $0x3,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1257>
mov    $0x3,%edi
mov    $0x1,%sil
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x126f>
lock addq $0x10,0x2a0(%rsp)
lock incq 0x2a8(%rsp)
lock incq 0x2b8(%rsp)
mov    0xe0(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x57f>
mov    (%rbx),%rax
mov    %rax,0xe0(%rsp)
movq   $0x0,(%rbx)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x686>
mov    0x330(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x810>
movzwl 0x340(%rsp),%ecx
cmp    %bpl,-0x1(%rax,%rcx,1)
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x7fe>
lea    -0x19(%rcx),%edx
cmp    $0xffe9,%dx
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x810>
cmp    $0x10,%cx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1228>
cmp    $0x2,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x810>
lea    0x4(%rcx),%edx
and    $0x1c,%edx
lea    0x3(%rcx),%esi
and    $0x3c,%esi
cmp    %esi,%edx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x810>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12ac>
mov    $0x1,%edi
xor    %ebx,%ebx
mov    $0x1,%sil
mov    %rdi,%rax
cmp    0x8(%rsp),%rdi
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12fd>
lea    (%r15,%rbx,8),%rsi
lea    (%r15,%rbx,8),%rdi
add    $0x8,%rdi
mov    0x8(%rsp),%rdx
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *0xa448e(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,(%r15,%rbx,8)
movq   $0x1,0x80(%rsp)
mov    %r12,0x88(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x18(%rsp)
mov    $0x1000,%eax
mov    %rax,(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa444a(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a38>
mov    0x360(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a38>
mov    $0x1000,%edx
mov    %rbx,%rdi
xor    %esi,%esi
call   *0xa4431(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2c0(%rsp),%rax
mov    %rax,(%rbx)
movq   $0x6,0x8(%rbx)
mov    %rbx,0x2c0(%rsp)
lea    0xff0(%rbx),%rax
mov    $0xffffffffffffff08,%rcx
lea    0x2d0(%rsp),%r13
mov    0xe0(%rsp),%rdx
mov    %rdx,(%rax)
mov    %rax,0xe0(%rsp)
lea    -0x10(%rax),%rdx
mov    0xe0(%rsp),%rsi
mov    %rsi,-0x10(%rax)
mov    %rdx,0xe0(%rsp)
lea    -0x20(%rax),%rdx
mov    0xe0(%rsp),%rsi
mov    %rsi,-0x20(%rax)
mov    %rdx,0xe0(%rsp)
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x67a>
lea    -0x30(%rax),%rdx
mov    0xe0(%rsp),%rsi
mov    %rsi,-0x30(%rax)
mov    %rdx,0xe0(%rsp)
add    $0xffffffffffffffc0,%rax
add    $0x4,%rcx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x610>
movq   $0x0,0x40(%rbx)
add    $0x40,%rbx
mov    %rbp,(%rbx)
movq   $0x1,0x80(%rsp)
movq   $0x1,0x88(%rsp)
mov    %rbx,0x90(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
mov    0x8(%rsp),%rax
xor    %ebx,%ebx
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6ed>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %rbx,%rcx
mov    %rax,%rdx
shr    $1,%rdx
add    %rdx,%rbx
cmp    %rbp,(%r15,%rbx,8)
cmova  %rcx,%rbx
sub    %rdx,%rax
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6d0>
cmp    %rbp,(%r15,%rbx,8)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
adc    $0x0,%rbx
mov    0x8(%rsp),%rcx
cmp    $0x1f,%rcx
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x456>
mov    %r15,%r13
xorps  %xmm0,%xmm0
movaps %xmm0,0x40(%rsp)
lea    0x0(,%rcx,8),%rbx
xor    %r15d,%r15d
lea    0x3a8(%rsp),%r12
nopl   0x0(%rax,%rax,1)
mov    0x0(%r13,%r15,1),%rdx
xorps  %xmm0,%xmm0
movaps %xmm0,0x390(%rsp)
movaps %xmm0,0x380(%rsp)
movaps %xmm0,0x370(%rsp)
movaps %xmm0,0x360(%rsp)
movq   $0xffffffffffffffff,0x3a0(%rsp)
movups %xmm0,0x1a(%r12)
movups %xmm0,0x10(%r12)
movups %xmm0,(%r12)
lea    0xa0(%rsp),%rdi
lea    0x40(%rsp),%rsi
mov    $0x8,%ecx
lea    0x360(%rsp),%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
add    $0x8,%r15
cmp    %r15,%rbx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x730>
lea    0xa0(%rsp),%rdi
lea    0x40(%rsp),%rsi
mov    %rbp,%rdx
mov    $0x8,%ecx
lea    0x2d0(%rsp),%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
mov    %r13,%rdi
movaps 0x40(%rsp),%xmm0
movaps %xmm0,0x360(%rsp)
mov    0x8(%rsp),%r15
cmp    $0x21,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe82>
lock subq $0x100,0x2a0(%rsp)
lock decq 0x2a8(%rsp)
mov    $0x100,%ebx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xea9>
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    0x320(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa56>
mov    0x338(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa30>
movq   $0x0,0x338(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa30>
movzbl 0x319(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x883>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c6f21(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa30>
movzbl 0x31a(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8c8>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c6f66(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa30>
movzbl 0x31b(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x90d>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c6fab(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa30>
movzbl 0x31c(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x952>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c6ff0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa30>
movzbl 0x31d(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x997>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c7035(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa30>
movzbl 0x31e(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x9dc>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c707a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa30>
movzbl 0x31f(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa1d>
mov    0x308(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c70bb(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a0e>
nopw   0x0(%rax,%rax,1)
movq   $0xffffffffffffffff,0x310(%rsp)
movw   $0x0,0x340(%rsp)
xorps  %xmm0,%xmm0
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
mov    0xa8(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa90>
lea    0xa0(%rsp),%rdi
lea    0x88(%rsp),%rsi
mov    %rbp,%rdx
mov    $0x8,%ecx
mov    %r13,%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
test   %al,%al
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
incq   0x98(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
nop
lea    0x80(%rsp),%rdi
mov    %rbp,%rsi
call   *0xa43b7(%rip)        # ADDR <_DYNAMIC+0x750>
mov    0x30(%rsp),%rax
mov    %rax,0x40(%rsp)
lea    0x40(%rsp),%rax
mov    0xa8(%rsp),%eax
mov    0x40(%rsp),%rdx
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd70>
mov    0x320(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd06>
mov    0x338(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xce0>
movq   $0x0,0x338(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xce0>
movzbl 0x319(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb37>
mov    0x2d8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c71d5(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xce0>
movzbl 0x31a(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb7c>
mov    0x2e0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c721a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xce0>
movzbl 0x31b(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xbc1>
mov    0x2e8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c725f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xce0>
movzbl 0x31c(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xc06>
mov    0x2f0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c72a4(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xce0>
movzbl 0x31d(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xc4b>
mov    0x2f8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c72e9(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xce0>
movzbl 0x31e(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xc90>
mov    0x300(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c732e(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xce0>
movzbl 0x31f(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xcd1>
mov    0x308(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c736f(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a0e>
nopl   0x0(%rax,%rax,1)
movq   $0xffffffffffffffff,0x310(%rsp)
movw   $0x0,0x340(%rsp)
xorps  %xmm0,%xmm0
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
mov    0x80(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
cmp    $0x1,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd90>
mov    0x88(%rsp),%r15
test   %r15,%r15
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
mov    0x90(%rsp),%rbx
cmp    $0x1,%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xdf8>
cmp    %rdx,(%rbx)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
lock subq $0x10,0x2a0(%rsp)
lock decq 0x2a8(%rsp)
mov    0xe0(%rsp),%rax
mov    %rax,(%rbx)
mov    %rbx,0xe0(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xde5>
lea    0x80(%rsp),%rdi
mov    %rdx,%rsi
call   *0xa40df(%rip)        # ADDR <_DYNAMIC+0x758>
mov    %eax,%ebx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
nopl   0x0(%rax,%rax,1)
lea    0xa0(%rsp),%rdi
lea    0x88(%rsp),%rsi
mov    $0x8,%ecx
call   ADDR <expanse_trie::mutate::remove::<false, false>>
test   %al,%al
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
mov    0x98(%rsp),%rax
dec    %rax
mov    %rax,0x98(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xde5>
mov    $0x1,%bl
cmp    $0x1e,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
lea    0x80(%rsp),%rdi
call   ADDR <<expanse_trie::set::ExpanseSet>::condense_to_root_leaf::<false>>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
movq   $0x0,0x80(%rsp)
mov    $0x1,%bl
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
mov    %r15,%rax
xor    %ebp,%ebp
nopl   (%rax)
mov    %rbp,%rcx
mov    %rax,%rsi
shr    $1,%rsi
add    %rsi,%rbp
cmp    %rdx,(%rbx,%rbp,8)
cmova  %rcx,%rbp
sub    %rsi,%rax
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe00>
cmp    %rdx,(%rbx,%rbp,8)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
lea    (%rbx,%rbp,8),%rdi
lea    -0x1(%r15),%rax
mov    %rax,0x30(%rsp)
cmp    $0x4,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe50>
xor    %ecx,%ecx
cmp    $0x3,%r15
setne  %cl
mov    $0x2,%edx
sub    %rcx,%rdx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x164d>
cmp    $0x12,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1184>
mov    $0x13,%edx
cmp    $0x11,%r15
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe7a>
lea    0x2(%r15),%rdx
lea    0x3(%r15),%rax
xor    %rdx,%rax
cmp    $0x4,%rax
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x160b>
and    $0x3c,%edx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x164b>
add    $0x18,%rbx
and    $0xffffffffffffffe0,%rbx
lock sub %rbx,0x2a0(%rsp)
lock decq 0x2a8(%rsp)
cmp    $0x178,%rbx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d8>
lea    -0x2c544a(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rbx,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11eb>
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19d9>
mov    0xb0(%rsp,%r13,8),%rax
mov    %rax,(%rdi)
mov    %rdi,0xb0(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11f1>
mov    0xa0(%rsp),%r15
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1970>
shl    $0x6,%rax
lock subq $0x40,(%r15,%rax,1)
lock decq 0x8(%r15,%rax,1)
mov    0x1000(%r15),%rdi
add    $0x40,%rdi
mov    $0x40,%edx
mov    $0x40,%ecx
mov    %rbx,%rsi
call   *0xa3d01(%rip)        # ADDR <_DYNAMIC+0x528>
mov    0x2d0(%rsp),%rax
xorps  %xmm0,%xmm0
movups %xmm0,(%rax)
movb   $0x7f,0xf(%rax)
movb   $0xff,0x8(%rax)
mov    0x320(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa85>
mov    0x338(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x115c>
movq   $0x0,0x338(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x115c>
movzbl 0x319(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xfb8>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c7656(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x115c>
movzbl 0x31a(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xffd>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c769b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x115c>
movzbl 0x31b(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1042>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c76e0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x115c>
movzbl 0x31c(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1087>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c7725(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x115c>
movzbl 0x31d(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x10cc>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c776a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x115c>
movzbl 0x31e(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1111>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c77af(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x115c>
movzbl 0x31f(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1152>
mov    0x308(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c77f0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a0e>
movq   $0xffffffffffffffff,0x310(%rsp)
movw   $0x0,0x340(%rsp)
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa85>
cmp    $0x1a,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1239>
cmp    $0x19,%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x160b>
mov    $0x18,%edx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x164b>
cmp    $0x18,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x127a>
cmp    $0x10,%rcx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1291>
mov    $0x18,%eax
mov    $0x11,%edi
mov    0x8(%rsp),%rcx
add    $0x3,%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1267>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1291>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%rbx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19b2>
call   *0xa3747(%rip)        # ADDR <free@GLIBC_2.2.5>
inc    %r15
mov    %r15,0x98(%rsp)
movaps 0x360(%rsp),%xmm0
lea    0x88(%rsp),%rax
movups %xmm0,(%rax)
movq   $0x2,0x80(%rsp)
lea    0x2d0(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
cmp    $0x10,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1299>
cmp    $0x18,%ecx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x810>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12ac>
cmp    $0x22,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x15fa>
cmp    $0x21,%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x160b>
mov    $0x20,%edx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x164b>
lea    0x4(%rcx),%eax
and    $0x1c,%eax
add    $0x3,%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1291>
cmp    $0x11,%rdi
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12ea>
xor    %esi,%esi
lea    0x3(%rdi),%eax
and    $0x3c,%eax
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12fd>
setne  %al
movzbl %al,%eax
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
cmp    %eax,%ecx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1267>
mov    %rdi,%r12
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x544>
mov    $0x18,%edx
lea    0x3(%rcx),%esi
and    $0x3c,%esi
cmp    %esi,%edx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x810>
mov    %bpl,(%rax,%rcx,1)
mov    0x2d0(%rsp),%rax
mov    0x8(%rax),%rdx
mov    $0xffffffffffffff00,%rsi
and    %rsi,%rdx
or     %rcx,%rdx
mov    %rdx,0x8(%rax)
incw   0x340(%rsp)
incq   0x338(%rsp)
incq   0x98(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
xor    %eax,%eax
cmp    $0x19,%rdi
setae  %al
lea    0x18(,%rax,8),%rax
xor    %esi,%esi
mov    %r15,0x10(%rsp)
lea    0x8(,%rax,8),%ecx
movabs $0x7ffffffffffffff0,%r15
and    %ecx,%r15d
lock add %r15,0x2a0(%rsp)
lock incq 0x2a8(%rsp)
lock incq 0x2b8(%rsp)
lea    0x0(,%rax,8),%edx
lea    -0x2c58d9(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rcx,1),%r13d
cmp    $0xff,%r13
mov    %rdi,0x60(%rsp)
mov    %esi,0x38(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1383>
cmp    $0x3d,%r13b
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19e7>
mov    0xb0(%rsp,%r13,8),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x13b3>
mov    (%r12),%rax
mov    %rax,0xb0(%rsp,%r13,8)
mov    0x10(%rsp),%r15
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14dd>
cmp    $0x1,%eax
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1490>
mov    $0x1,%esi
mov    %rdx,%rdi
mov    %rdx,%r13
call   *0xa366b(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r12
test   %rax,%rax
mov    0x10(%rsp),%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14e8>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a2a>
mov    %rdx,0x348(%rsp)
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x18(%rsp)
mov    $0x1000,%eax
mov    %rax,(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa360e(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a38>
mov    0x360(%rsp),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a38>
mov    $0x1000,%edx
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa35f5(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2c0(%rsp),%rax
mov    %rax,(%r12)
mov    %r13,0x8(%r12)
mov    %r12,0x2c0(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r15w
movzwl %ax,%eax
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %r15,%rcx
lea    (%r12,%rcx,1),%rdx
add    $0x40,%rdx
mov    0xb0(%rsp,%r13,8),%rsi
mov    %rsi,0x40(%r12,%rcx,1)
mov    %rdx,0xb0(%rsp,%r13,8)
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1450>
add    $0x40,%r12
mov    0x10(%rsp),%r15
mov    0x348(%rsp),%rdx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14dd>
movq   $0x0,0x360(%rsp)
mov    $0x10,%eax
mov    %rax,0x18(%rsp)
mov    $0x10,%esi
lea    0x360(%rsp),%rdi
mov    %rdx,%r13
call   *0xa3544(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a34>
mov    0x360(%rsp),%r12
test   %r12,%r12
mov    0x10(%rsp),%r15
mov    %r13,%rdx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a08>
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa3528(%rip)        # ADDR <memset@GLIBC_2.2.5>
lea    0x0(,%rbx,8),%rdx
mov    %r12,%rdi
mov    %r15,%rsi
mov    0xa3423(%rip),%rax        # ADDR <memcpy@GLIBC_2.14>
mov    %r15,%r13
mov    %rax,%r15
call   *%rax
mov    %rbp,(%r12,%rbx,8)
lea    (%r12,%rbx,8),%rdi
add    $0x8,%rdi
lea    0x0(,%rbx,8),%rsi
add    %r13,%rsi
mov    0x8(%rsp),%r13
mov    %r13,%rdx
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *%r15
cmpb   $0x0,0x38(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x153c>
mov    0x10(%rsp),%rdi
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x154f>
cmp    $0x11,%r13
mov    0x10(%rsp),%rdi
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x156a>
add    $0x3,%r13d
and    $0x3c,%r13d
lea    0x8(,%r13,8),%rax
shl    $0x3,%r13
movabs $0x7ffffffffffffff0,%rcx
and    %rcx,%rax
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x157f>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
add    $0xc0,%rax
mov    %rax,%r13
lock sub %rax,0x2a0(%rsp)
lock decq 0x2a8(%rsp)
lea    -0x2c5b32(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl 0x0(%r13,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x15c6>
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19d9>
mov    0xb0(%rsp,%r13,8),%rax
mov    %rax,(%rdi)
mov    %rdi,0xb0(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x15cc>
call   *0xa336c(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x60(%rsp),%rax
movq   $0x1,0x80(%rsp)
mov    %rax,0x88(%rsp)
mov    %r12,0x90(%rsp)
lea    0x2d0(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa1>
lea    0x2(%r15),%rdx
lea    0x3(%r15),%rax
xor    %rdx,%rax
cmp    $0x4,%rax
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1647>
lea    0x8(%rdi),%rsi
mov    0x30(%rsp),%r15
mov    %r15,%rdx
sub    %rbp,%rdx
shl    $0x3,%rdx
call   *0xa33cc(%rip)        # ADDR <memmove@GLIBC_2.2.5>
movq   $0x1,0x80(%rsp)
mov    %r15,0x88(%rsp)
mov    %rbx,0x90(%rsp)
mov    $0x1,%bl
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
and    $0xfffffffffffffffc,%rdx
xor    %ecx,%ecx
lea    0x8(,%rdx,8),%rsi
and    $0xfffffffffffffff0,%rsi
lock add %rsi,0x2a0(%rsp)
shl    $0x3,%rdx
lock incq 0x2a8(%rsp)
lock incq 0x2b8(%rsp)
cmp    $0x177,%rdx
mov    %rcx,0x8(%rsp)
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x16c7>
lea    -0x2c5c27(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x16fc>
cmp    $0x3d,%r13b
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19e7>
mov    0xb0(%rsp,%r13,8),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x174f>
mov    (%r12),%rax
mov    %rax,0xb0(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1836>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19b2>
mov    $0x1,%esi
mov    %rdx,%rdi
mov    %rdx,%r13
call   *0xa331d(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r12
test   %rax,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1841>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a2a>
cmp    $0xf,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x16da>
movq   $0x0,0x360(%rsp)
mov    $0x10,%eax
mov    %rax,0x18(%rsp)
mov    $0x10,%esi
lea    0x360(%rsp),%rdi
mov    %rdx,%r13
call   *0xa32d2(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a34>
mov    0x360(%rsp),%r12
test   %r12,%r12
mov    %r13,%rdx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1836>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a08>
cmp    $0x101,%rdx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x16da>
mov    %rsi,0x10(%rsp)
mov    %rdx,0x38(%rsp)
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x18(%rsp)
mov    $0x1000,%eax
mov    %rax,(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa3267(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a38>
mov    0x360(%rsp),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a38>
mov    $0x1000,%edx
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa324e(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2c0(%rsp),%rax
mov    %rax,(%r12)
mov    %r13,0x8(%r12)
mov    %r12,0x2c0(%rsp)
mov    0x10(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a1b>
cmp    $0x7e0,%rdi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x182d>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %di
movzwl %ax,%eax
xchg   %ax,%ax
dec    %rax
mov    %rax,%rcx
imul   %rdi,%rcx
lea    (%r12,%rcx,1),%rdx
add    $0x40,%rdx
mov    0xb0(%rsp,%r13,8),%rsi
mov    %rsi,0x40(%r12,%rcx,1)
mov    %rdx,0xb0(%rsp,%r13,8)
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1800>
add    $0x40,%r12
mov    0x38(%rsp),%rdx
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa31cf(%rip)        # ADDR <memset@GLIBC_2.2.5>
lea    0x0(,%rbp,8),%rdx
mov    %r12,%rdi
mov    %rbx,%rsi
mov    0xa30ca(%rip),%r13        # ADDR <memcpy@GLIBC_2.14>
call   *%r13
lea    (%r12,%rbp,8),%rdi
lea    (%rbx,%rbp,8),%rsi
add    $0x8,%rsi
lea    -0x1(%r15),%rdx
sub    %rbp,%rdx
shl    $0x3,%rdx
call   *%r13
cmpb   $0x0,0x8(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1888>
cmp    $0x11,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18c7>
add    $0x3,%r15d
and    $0x3c,%r15d
lea    0x8(,%r15,8),%rax
shl    $0x3,%r15
and    $0xfffffffffffffff0,%rax
lock sub %rax,0x2a0(%rsp)
lock decq 0x2a8(%rsp)
cmp    $0x177,%r15
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18f6>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r15
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x192a>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19b2>
mov    $0xc0,%eax
cmp    $0x19,%r15
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18e1>
mov    $0x100,%eax
cmp    $0x21,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1963>
lock sub %rax,0x2a0(%rsp)
lock decq 0x2a8(%rsp)
mov    %rax,%r15
lea    -0x2c5e97(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%r15,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x192a>
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19d9>
mov    0xb0(%rsp,%r13,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0xb0(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1933>
mov    %rbx,%rdi
call   *0xa3005(%rip)        # ADDR <free@GLIBC_2.2.5>
movq   $0x1,0x80(%rsp)
mov    0x30(%rsp),%rax
mov    %rax,0x88(%rsp)
mov    %r12,0x90(%rsp)
mov    $0x1,%bl
lea    0x2d0(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
add    $0x3,%r15
and    $0xfffffffffffffffc,%r15
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1888>
call   *0xa32c2(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf01>
mov    %r15,0x70(%rsp)
lea    0x70(%rsp),%rax
mov    0x68(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x199b>
mov    0x58(%rsp),%rdi
call   *0xa2f9d(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x70(%rsp),%rax
add    $0x3d8,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
lea    -0x2c8104(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0xa0320(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x994a9(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x27(%rsp),%rdx
mov    $0x11,%esi
call   *0xa2ff9(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a47>
mov    $0x3e,%esi
lea    0x99473(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19f3>
mov    $0x3e,%esi
lea    0x99495(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    %rax,0x28(%rsp)
mov    %r13,%rdi
mov    0x28(%rsp),%rdx
call   *0xa2f0a(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a47>
mov    %rdx,(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a38>
mov    $0x8,%r13d
mov    $0x8,%esi
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19f8>
lea    0x99466(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
call   *0xa30f8(%rip)        # ADDR <_DYNAMIC+0x418>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a47>
mov    $0x10,%eax
mov    %rax,0x18(%rsp)
mov    %r13,(%rsp)
mov    0x18(%rsp),%rdi
mov    (%rsp),%rsi
call   *0xa2f41(%rip)        # ADDR <_DYNAMIC+0x280>
ud2
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a4d>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a4d>
mov    %rax,%rbx
lea    0x80(%rsp),%rdi
call   ADDR <core::ptr::drop_glue::<expanse_trie::set::ExpanseSet>>
mov    0x68(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a73>
mov    0x58(%rsp),%rdi
call   *0xa2ec5(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
call   *0xa2f1f(%rip)        # ADDR <_DYNAMIC+0x298>
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
