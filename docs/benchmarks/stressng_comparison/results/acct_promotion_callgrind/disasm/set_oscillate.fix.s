push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x3d8,%rsp
mov    %rdi,%rbx
lea    0xb0(%rsp),%rdi
mov    $0x1f0,%edx
xor    %esi,%esi
call   *0xa5727(%rip)        # ADDR <memset@GLIBC_2.2.5>
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
mov    0x8(%rbx),%r12
mov    %rbx,0x68(%rsp)
mov    0x10(%rbx),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ad3>
lea    0x2d0(%rsp),%r13
lea    (%r12,%rax,8),%rax
mov    %rax,0x348(%rsp)
lea    0xa2bf4(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
mov    %rax,0x30(%rsp)
xor    %eax,%eax
mov    %r12,%r9
mov    %r12,0x38(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d>
nopl   (%rax)
xor    %ebx,%ebx
mov    0x350(%rsp),%r9
add    $0x8,%r9
movzbl %bl,%eax
mov    0x358(%rsp),%rcx
add    %rax,%rcx
mov    %rcx,%rax
cmp    0x348(%rsp),%r9
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ad5>
mov    %rax,0x358(%rsp)
mov    (%r9),%rax
mov    %rax,0x48(%rsp)
mov    %rax,0x78(%rsp)
lea    0x78(%rsp),%rax
mov    0x78(%rsp),%rbp
cmpl   $0x2,0x80(%rsp)
mov    %r9,0x350(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x410>
mov    %rbp,%rax
shr    $0x8,%rax
cmp    %rax,0x310(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x890>
mov    0x328(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x4e4>
mov    %ebp,%eax
shr    $0x3,%eax
and    $0x18,%eax
mov    (%rbx,%rax,1),%rcx
bts    %rbp,%rcx
mov    %rcx,(%rbx,%rax,1)
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
incq   0x338(%rsp)
mov    0x340(%rsp),%eax
lea    0x1(%rax),%ecx
mov    %cx,0x340(%rsp)
mov    0x2d0(%rsp),%rcx
mov    %al,0x8(%rcx)
cmpw   $0x100,0x340(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb05>
mov    0x338(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3d5>
movq   $0x0,0x338(%rsp)
mov    0x320(%rsp),%rax
cmp    $0x2,%rax
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3d5>
movzbl 0x319(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x231>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cbf9f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3d5>
movzbl 0x31a(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x276>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cbfe4(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3d5>
movzbl 0x31b(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x2bb>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc029(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3d5>
movzbl 0x31c(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x300>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc06e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3d5>
movzbl 0x31d(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x345>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc0b3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3d5>
movzbl 0x31e(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x38a>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc0f8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3d5>
movzbl 0x31f(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3cb>
mov    0x308(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc139(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b6f>
mov    0xa8(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xfad>
addq   $0xffffffffffffffc0,0x2a0(%rsp)
decq   0x2a8(%rsp)
mov    0xb8(%rsp),%rax
mov    %rax,(%rbx)
mov    %rbx,0xb8(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xff8>
nopl   (%rax)
mov    0xa8(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb10>
cmpq   $0x0,0x80(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x4a3>
mov    0x88(%rsp),%rcx
mov    0x90(%rsp),%r15
mov    $0x1,%eax
mov    %rax,0x10(%rsp)
test   %rcx,%rcx
mov    %rcx,0x8(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x542>
mov    %rcx,%rbx
cmp    -0x8(%r15,%rcx,8),%rbp
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6ce>
cmp    $0x1f,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x72a>
mov    $0x2,%eax
cmp    $0x1,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x544>
lea    0x1(%rcx),%rax
mov    %rax,0x10(%rsp)
cmp    $0x10,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1263>
cmp    $0x3,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x163b>
mov    $0x3,%eax
mov    $0x1,%sil
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1660>
addq   $0x10,0x2a0(%rsp)
incq   0x2a8(%rsp)
incq   0x2b8(%rsp)
mov    0xe0(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x593>
mov    (%rbx),%rax
mov    %rax,0xe0(%rsp)
movq   $0x0,(%rbx)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6a6>
mov    0x330(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x890>
movzwl 0x340(%rsp),%ecx
cmp    %bpl,-0x1(%rax,%rcx,1)
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x87e>
lea    -0x19(%rcx),%edx
cmp    $0xffe9,%dx
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x890>
cmp    $0x10,%cx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x129e>
cmp    $0x2,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x890>
lea    0x4(%rcx),%edx
and    $0x1c,%edx
lea    0x3(%rcx),%esi
and    $0x3c,%esi
cmp    %esi,%edx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x890>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1919>
xor    %ebx,%ebx
mov    $0x1,%sil
mov    %rax,%rcx
mov    0x8(%rsp),%rdx
cmp    %rdx,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1666>
lea    (%r15,%rbx,8),%rsi
lea    (%r15,%rbx,8),%rdi
add    $0x8,%rdi
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *0xa51bf(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,(%r15,%rbx,8)
movq   $0x1,0x80(%rsp)
mov    0x10(%rsp),%rax
mov    %rax,0x88(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x20(%rsp)
mov    $0x1000,%eax
mov    %rax,0x18(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa5175(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b90>
mov    0x360(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b90>
mov    $0x1000,%edx
mov    %rbx,%rdi
xor    %esi,%esi
call   *0xa515c(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2c0(%rsp),%rax
mov    %rax,(%rbx)
movq   $0x6,0x8(%rbx)
mov    %rbx,0x2c0(%rsp)
lea    0xff0(%rbx),%rax
mov    $0xffffffffffffff08,%rcx
mov    0x38(%rsp),%r12
lea    0x2d0(%rsp),%r13
nopw   0x0(%rax,%rax,1)
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
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x69a>
lea    -0x30(%rax),%rdx
mov    0xe0(%rsp),%rsi
mov    %rsi,-0x30(%rax)
mov    %rdx,0xe0(%rsp)
add    $0xffffffffffffffc0,%rax
add    $0x4,%rcx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x630>
movq   $0x0,0x40(%rbx)
add    $0x40,%rbx
mov    %rbp,(%rbx)
movq   $0x1,0x80(%rsp)
movq   $0x1,0x88(%rsp)
mov    %rbx,0x90(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
mov    0x8(%rsp),%rax
xor    %ebx,%ebx
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x70d>
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %rbx,%rcx
mov    %rax,%rdx
shr    $1,%rdx
add    %rdx,%rbx
cmp    %rbp,(%r15,%rbx,8)
cmova  %rcx,%rbx
sub    %rdx,%rax
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6f0>
cmp    %rbp,(%r15,%rbx,8)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
adc    $0x0,%rbx
mov    0x8(%rsp),%rcx
cmp    $0x1f,%rcx
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x46a>
xorps  %xmm0,%xmm0
movaps %xmm0,0x50(%rsp)
lea    0x0(,%rcx,8),%rbx
xor    %r14d,%r14d
nopl   (%rax)
mov    (%r15,%r14,1),%rdx
xorps  %xmm0,%xmm0
movaps %xmm0,0x390(%rsp)
movaps %xmm0,0x380(%rsp)
movaps %xmm0,0x370(%rsp)
movaps %xmm0,0x360(%rsp)
movq   $0xffffffffffffffff,0x3a0(%rsp)
lea    0x3a8(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0xa0(%rsp),%rdi
lea    0x50(%rsp),%rsi
mov    $0x8,%ecx
lea    0x360(%rsp),%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
add    $0x8,%r14
cmp    %r14,%rbx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x740>
lea    0xa0(%rsp),%rdi
lea    0x50(%rsp),%rsi
mov    %rbp,%rdx
mov    $0x8,%ecx
mov    %r13,%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
movaps 0x50(%rsp),%xmm0
movaps %xmm0,0x360(%rsp)
mov    $0x100,%r14d
mov    0x8(%rsp),%rbp
cmp    $0x21,%rbp
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x801>
add    $0x18,%rbx
mov    %rbx,%r14
and    $0xffffffffffffffe0,%r14
cmp    $0x177,%r14
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a06>
lea    -0x2ca470(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%r14,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a8b>
sub    %r14,0x2a0(%rsp)
decq   0x2a8(%rsp)
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b31>
mov    0xb0(%rsp,%r13,8),%rax
mov    %rax,(%r15)
mov    %r15,0xb0(%rsp,%r13,8)
lea    0x2d0(%rsp),%r13
inc    %rbp
mov    %rbp,0x98(%rsp)
movaps 0x360(%rsp),%xmm0
lea    0x88(%rsp),%rax
movups %xmm0,(%rax)
movq   $0x2,0x80(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    0x320(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xad6>
mov    0x338(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
movq   $0x0,0x338(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
movzbl 0x319(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x903>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc671(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
movzbl 0x31a(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x948>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc6b6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
movzbl 0x31b(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x98d>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc6fb(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
movzbl 0x31c(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x9d2>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc740(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
movzbl 0x31d(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa17>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc785(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
movzbl 0x31e(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa5c>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc7ca(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
movzbl 0x31f(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa9d>
mov    0x308(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cc80b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b6f>
nopw   0x0(%rax,%rax,1)
movq   $0xffffffffffffffff,0x310(%rsp)
movw   $0x0,0x340(%rsp)
xorps  %xmm0,%xmm0
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
mov    0xa8(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb10>
lea    0xa0(%rsp),%rdi
lea    0x88(%rsp),%rsi
mov    %rbp,%rdx
mov    $0x8,%ecx
mov    %r13,%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
test   %al,%al
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
incq   0x98(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
nop
lea    0x80(%rsp),%rdi
mov    %rbp,%rsi
call   *0xa5087(%rip)        # ADDR <_DYNAMIC+0x760>
mov    0x48(%rsp),%rax
mov    %rax,0x50(%rsp)
lea    0x50(%rsp),%rax
mov    0xa8(%rsp),%eax
mov    0x50(%rsp),%rdx
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xdf0>
mov    0x320(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd86>
mov    0x338(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd60>
movq   $0x0,0x338(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd60>
movzbl 0x319(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xbb7>
mov    0x2d8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2cc925(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd60>
movzbl 0x31a(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xbfc>
mov    0x2e0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2cc96a(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd60>
movzbl 0x31b(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xc41>
mov    0x2e8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2cc9af(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd60>
movzbl 0x31c(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xc86>
mov    0x2f0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2cc9f4(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd60>
movzbl 0x31d(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xccb>
mov    0x2f8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2cca39(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd60>
movzbl 0x31e(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd10>
mov    0x300(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2cca7e(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd60>
movzbl 0x31f(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd51>
mov    0x308(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2ccabf(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b6f>
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
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe10>
mov    0x88(%rsp),%r14
test   %r14,%r14
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
mov    0x90(%rsp),%r15
cmp    $0x1,%r14
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe78>
cmp    %rdx,(%r15)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
addq   $0xfffffffffffffff0,0x2a0(%rsp)
decq   0x2a8(%rsp)
mov    0xe0(%rsp),%rax
mov    %rax,(%r15)
mov    %r15,0xe0(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe65>
xchg   %ax,%ax
lea    0x80(%rsp),%rdi
mov    %rdx,%rsi
call   *0xa4daf(%rip)        # ADDR <_DYNAMIC+0x768>
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
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe65>
mov    $0x1,%bl
cmp    $0x1e,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
lea    0x80(%rsp),%rdi
call   ADDR <<expanse_trie::set::ExpanseSet>::condense_to_root_leaf::<false>>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
movq   $0x0,0x80(%rsp)
mov    $0x1,%bl
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
mov    %r14,%rax
xor    %ebx,%ebx
nopl   (%rax)
mov    %rbx,%rcx
mov    %rax,%rsi
shr    $1,%rsi
add    %rsi,%rbx
cmp    %rdx,(%r15,%rbx,8)
cmova  %rcx,%rbx
sub    %rsi,%rax
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe80>
cmp    %rdx,(%r15,%rbx,8)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
lea    -0x1(%r14),%rax
mov    %rax,0x48(%rsp)
cmp    $0x4,%r14
setb   %al
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xedf>
mov    $0x8,%ecx
cmp    $0x3,%r14
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12cf>
mov    $0x10,%ebp
movl   $0x0,0x10(%rsp)
mov    $0x10,%ecx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12dc>
cmp    $0x12,%r14
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x124d>
mov    $0x13,%ebp
cmp    $0x11,%r14
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf09>
lea    0x2(%r14),%rbp
lea    0x3(%r14),%rax
xor    %rbp,%rax
cmp    $0x4,%rax
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x198e>
and    $0x3c,%ebp
shl    $0x3,%rbp
movl   $0x0,0x10(%rsp)
mov    %rbp,%rcx
cmp    $0x177,%rbp
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12dc>
mov    %rcx,0x8(%rsp)
lea    0xa0(%rsp),%rdi
mov    %rbp,%rsi
call   *0xa4a0a(%rip)        # ADDR <_DYNAMIC+0x500>
movabs $0x7ffffffffffffff0,%rax
mov    0x8(%rsp),%rdx
cmp    %rax,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b0a>
cmp    $0xf,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1503>
movq   $0x0,0x360(%rsp)
mov    $0x10,%esi
lea    0x360(%rsp),%rdi
call   *0xa47c1(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b7c>
mov    0x360(%rsp),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b7c>
mov    %r12,%rdi
xor    %esi,%esi
mov    0x8(%rsp),%rdx
call   *0xa47a8(%rip)        # ADDR <memset@GLIBC_2.2.5>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1522>
mov    0xa0(%rsp),%r14
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a3d>
shl    $0x6,%rax
lock subq $0x40,(%r14,%rax,1)
lock decq 0x8(%r14,%rax,1)
mov    0x1000(%r14),%rdi
add    $0x40,%rdi
mov    $0x40,%edx
mov    $0x40,%ecx
mov    %rbx,%rsi
call   *0xa4980(%rip)        # ADDR <_DYNAMIC+0x530>
mov    0x2d0(%rsp),%rax
xorps  %xmm0,%xmm0
movups %xmm0,(%rax)
movb   $0x7f,0xf(%rax)
movb   $0xff,0x8(%rax)
mov    0x320(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb05>
mov    0x338(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1225>
movq   $0x0,0x338(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1225>
movzbl 0x319(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1081>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ccdef(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1225>
movzbl 0x31a(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x10c6>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cce34(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1225>
movzbl 0x31b(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x110b>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cce79(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1225>
movzbl 0x31c(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1150>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ccebe(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1225>
movzbl 0x31d(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1195>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ccf03(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1225>
movzbl 0x31e(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11da>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ccf48(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1225>
movzbl 0x31f(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x121b>
mov    0x308(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ccf89(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b6f>
movq   $0xffffffffffffffff,0x310(%rsp)
movw   $0x0,0x340(%rsp)
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb05>
cmp    $0x1a,%r14
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12b6>
mov    $0xc0,%ecx
cmp    $0x19,%r14
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12cf>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x198e>
cmp    $0x18,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18e1>
cmp    $0x10,%rcx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18fc>
mov    $0x18,%eax
mov    $0x11,%ecx
mov    %rcx,0x10(%rsp)
mov    0x8(%rsp),%rcx
add    $0x3,%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x164f>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18fc>
cmp    $0x10,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1906>
cmp    $0x18,%ecx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x890>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1919>
cmp    $0x22,%r14
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x197d>
mov    $0x100,%ecx
cmp    $0x21,%r14
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x198e>
mov    %eax,0x10(%rsp)
lea    0x8(%rcx),%ebp
and    $0x1d0,%ebp
lea    -0x2caf4b(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
mov    %rcx,0x8(%rsp)
movzbl (%rcx,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ac1>
add    %rbp,0x2a0(%rsp)
incq   0x2a8(%rsp)
incq   0x2b8(%rsp)
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b3f>
mov    0xb0(%rsp,%r13,8),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x133a>
mov    (%r12),%rax
mov    %rax,0xb0(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14e1>
cmpq   $0x101,0x8(%rsp)
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14fb>
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x20(%rsp)
mov    $0x1000,%eax
mov    %rax,0x18(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa43bf(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b90>
mov    0x360(%rsp),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b90>
mov    $0x1000,%edx
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa43a6(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2c0(%rsp),%rax
mov    %rax,(%r12)
mov    %r13,0x8(%r12)
mov    %r12,0x2c0(%rsp)
test   %rbp,%rbp
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b60>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %bp
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x141c>
mov    %rcx,%rax
nopw   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rsi
imul   %rbp,%rsi
lea    (%r12,%rsi,1),%rdi
add    $0x40,%rdi
mov    0xb0(%rsp,%r13,8),%r8
mov    %r8,0x40(%r12,%rsi,1)
mov    %rdi,0xb0(%rsp,%r13,8)
dec    %rdx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x13f0>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14dd>
lea    -0x4(%rax),%rcx
imul   %rbp,%rcx
add    $0x40,%rcx
lea    0x0(,%rbp,4),%rdx
neg    %rdx
lea    -0x3(%rax),%rsi
imul   %rbp,%rsi
add    $0x40,%rsi
lea    -0x2(%rax),%rdi
imul   %rbp,%rdi
add    $0x40,%rdi
lea    -0x1(%rax),%r8
imul   %rbp,%r8
add    $0x40,%r8
mov    %r12,%r9
nopw   0x0(%rax,%rax,1)
lea    (%r9,%r8,1),%r10
mov    0xb0(%rsp,%r13,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0xb0(%rsp,%r13,8)
lea    (%r9,%rdi,1),%r10
mov    0xb0(%rsp,%r13,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0xb0(%rsp,%r13,8)
lea    (%r9,%rsi,1),%r10
mov    0xb0(%rsp,%r13,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0xb0(%rsp,%r13,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0xb0(%rsp,%r13,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0xb0(%rsp,%r13,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1470>
add    $0x40,%r12
mov    %r12,%rdi
xor    %esi,%esi
mov    0x8(%rsp),%rdx
call   *0xa425f(%rip)        # ADDR <memset@GLIBC_2.2.5>
lea    0x2d0(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1522>
lea    0x2d0(%rsp),%r13
mov    $0x1,%esi
mov    0x8(%rsp),%rbp
mov    %rbp,%rdi
call   *0xa4232(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r12
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ba2>
lea    0x0(,%rbx,8),%rdx
mov    %r12,%rdi
mov    %r15,%rsi
mov    0xa4129(%rip),%rbp        # ADDR <memcpy@GLIBC_2.14>
call   *%rbp
lea    (%r12,%rbx,8),%rdi
lea    (%r15,%rbx,8),%rsi
add    $0x8,%rsi
lea    -0x1(%r14),%rdx
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *%rbp
cmpb   $0x0,0x10(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x156b>
cmp    $0x11,%r14
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1606>
add    $0x3,%r14d
and    $0x3c,%r14d
lea    0x8(,%r14,8),%rsi
mov    %r14,%rbx
shl    $0x3,%rbx
and    $0xfffffffffffffff0,%rsi
cmp    $0x177,%rbx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19ce>
lea    -0x2cb1fa(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rbx,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1aa6>
sub    %rsi,0x2a0(%rsp)
decq   0x2a8(%rsp)
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b31>
mov    0xb0(%rsp,%r13,8),%rax
mov    %rax,(%r15)
mov    %r15,0xb0(%rsp,%r13,8)
lea    0x2d0(%rsp),%r13
movq   $0x1,0x80(%rsp)
mov    0x48(%rsp),%rax
mov    %rax,0x88(%rsp)
mov    %r12,0x90(%rsp)
mov    $0x1,%bl
mov    0x38(%rsp),%r12
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
mov    $0xc0,%esi
mov    $0xc0,%ebx
cmp    $0x19,%r14
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x158b>
mov    $0x100,%esi
mov    $0x100,%ebx
cmp    $0x21,%r14
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x158b>
add    $0x3,%r14
and    $0xfffffffffffffffc,%r14
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x156b>
lea    0x4(%rcx),%eax
and    $0x1c,%eax
add    $0x3,%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18fc>
mov    0x10(%rsp),%rax
cmp    $0x11,%rax
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1957>
xor    %esi,%esi
lea    0x3(%rax),%ecx
and    $0x3c,%ecx
mov    %esi,0x44(%rsp)
lea    0x0(,%rcx,8),%edx
lea    0x8(,%rcx,8),%ecx
movabs $0x7ffffffffffffff0,%r14
and    %ecx,%r14d
mov    %rax,0x10(%rsp)
lea    -0x2cb2f9(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
mov    %rdx,0x60(%rsp)
movzbl (%rdx,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a48>
add    %r14,0x2a0(%rsp)
incq   0x2a8(%rsp)
incq   0x2b8(%rsp)
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b3f>
mov    0xb0(%rsp,%r13,8),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x16e8>
mov    (%r12),%rax
mov    %rax,0xb0(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17a1>
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x20(%rsp)
mov    $0x1000,%eax
mov    %rax,0x18(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa4020(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b90>
mov    0x360(%rsp),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b90>
mov    $0x1000,%edx
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa4007(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2c0(%rsp),%rax
mov    %rax,(%r12)
mov    %r13,0x8(%r12)
mov    %r12,0x2c0(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r14w
movzwl %ax,%eax
nop
dec    %rax
mov    %rax,%rcx
imul   %r14,%rcx
lea    (%r12,%rcx,1),%rdx
add    $0x40,%rdx
mov    0xb0(%rsp,%r13,8),%rsi
mov    %rsi,0x40(%r12,%rcx,1)
mov    %rdx,0xb0(%rsp,%r13,8)
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1770>
add    $0x40,%r12
mov    %r12,%rdi
xor    %esi,%esi
mov    0x60(%rsp),%rdx
call   *0xa3f9f(%rip)        # ADDR <memset@GLIBC_2.2.5>
lea    0x0(,%rbx,8),%rdx
mov    %r12,%rdi
mov    %r15,%rsi
mov    0xa3e9a(%rip),%r14        # ADDR <memcpy@GLIBC_2.14>
call   *%r14
mov    %rbp,(%r12,%rbx,8)
lea    (%r12,%rbx,8),%rdi
add    $0x8,%rdi
lea    (%r15,%rbx,8),%rsi
mov    0x8(%rsp),%r13
mov    %r13,%rdx
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *%r14
cmpb   $0x0,0x44(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1804>
cmp    $0x11,%r13
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1897>
add    $0x3,%r13d
and    $0x3c,%r13d
lea    0x8(,%r13,8),%rsi
shl    $0x3,%r13
movabs $0x7ffffffffffffff0,%rax
and    %rax,%rsi
lea    -0x2cb48c(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl 0x0(%r13,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18c8>
sub    %rsi,0x2a0(%rsp)
decq   0x2a8(%rsp)
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b31>
mov    0xb0(%rsp,%r13,8),%rax
mov    %rax,(%r15)
mov    %r15,0xb0(%rsp,%r13,8)
movq   $0x1,0x80(%rsp)
mov    0x10(%rsp),%rax
mov    %rax,0x88(%rsp)
mov    %r12,0x90(%rsp)
mov    0x38(%rsp),%r12
lea    0x2d0(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
xor    %esi,%esi
cmp    $0x19,%r13
setae  %sil
shl    $0x6,%esi
add    $0xc0,%rsi
mov    %rsi,%r13
lea    -0x2cb51d(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl 0x0(%r13,%rax,1),%r13d
cmp    $0xff,%r13
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1837>
lea    0xa0(%rsp),%rdi
call   *0xa40aa(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %r15,%rdi
call   *0xa3d99(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1864>
setne  %al
movzbl %al,%eax
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
cmp    %eax,%ecx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x164f>
mov    0x8(%rsp),%rdx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x558>
mov    $0x18,%edx
lea    0x3(%rcx),%esi
and    $0x3c,%esi
cmp    %esi,%edx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x890>
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
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb21>
xor    %r14d,%r14d
cmp    $0x19,%rax
setae  %r14b
shl    $0x6,%r14d
add    $0xc0,%r14
movl   $0x0,0x44(%rsp)
mov    %r14,%rdx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x168a>
lea    0x2(%r14),%rbp
lea    0x3(%r14),%rax
xor    %rbp,%rax
cmp    $0x4,%rax
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19fd>
lea    (%r15,%rbx,8),%rdi
lea    0x8(%rdi),%rsi
mov    0x48(%rsp),%r14
mov    %r14,%rdx
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *0xa3d85(%rip)        # ADDR <memmove@GLIBC_2.2.5>
movq   $0x1,0x80(%rsp)
mov    %r14,0x88(%rsp)
mov    %r15,0x90(%rsp)
mov    $0x1,%bl
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
lea    0xa0(%rsp),%rdi
call   *0xa3fa4(%rip)        # ADDR <_DYNAMIC+0x538>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%rbx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b0a>
mov    %r15,%rdi
call   *0xa3c80(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x15d9>
and    $0xfffffffffffffffc,%rbp
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0c>
lea    0xa0(%rsp),%rdi
mov    %r14,%rsi
call   *0xa3f69(%rip)        # ADDR <_DYNAMIC+0x538>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r14
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b0a>
mov    %r15,%rdi
call   *0xa3c45(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x38(%rsp),%r12
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x84f>
call   *0xa3f45(%rip)        # ADDR <_DYNAMIC+0x540>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xfca>
lea    0xa0(%rsp),%rdi
mov    %r14,%rsi
call   *0xa3eef(%rip)        # ADDR <_DYNAMIC+0x500>
mov    $0x10,%eax
mov    %rax,0x20(%rsp)
mov    $0x10,%esi
mov    0x60(%rsp),%r14
mov    %r14,%rdi
call   ADDR <__rustc::__rust_alloc_zeroed>
mov    %rax,%r12
mov    %r14,0x18(%rsp)
test   %rax,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17b1>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b90>
lea    0xa0(%rsp),%rdi
mov    %r14,%rsi
call   *0xa3ee4(%rip)        # ADDR <_DYNAMIC+0x538>
lea    0x2d0(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a2a>
lea    0xa0(%rsp),%rdi
call   *0xa3ecc(%rip)        # ADDR <_DYNAMIC+0x538>
lea    0x2d0(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19ef>
lea    0x2d0(%rsp),%r13
mov    0x8(%rsp),%rcx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf28>
xor    %eax,%eax
mov    %rax,0x70(%rsp)
lea    0x70(%rsp),%rax
mov    0x68(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1af3>
mov    %r12,%rdi
call   *0xa3b85(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x70(%rsp),%rax
add    $0x3d8,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
lea    -0x2cd92c(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0xa0f08(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x9a091(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x2f(%rsp),%rdx
mov    $0x11,%esi
call   *0xa3be1(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ba0>
mov    $0x3e,%esi
lea    0x9a05b(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b4b>
mov    $0x3e,%esi
lea    0x9a07d(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    %rax,0x30(%rsp)
mov    %r13,%rdi
mov    0x30(%rsp),%rdx
call   *0xa3af2(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ba0>
lea    0x9a061(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
call   *0xa3cf3(%rip)        # ADDR <_DYNAMIC+0x418>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ba0>
mov    $0x8,%r13d
mov    $0x8,%esi
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b50>
mov    $0x10,%eax
mov    %rax,0x20(%rsp)
mov    0x8(%rsp),%rax
mov    %rax,0x18(%rsp)
mov    0x20(%rsp),%rdi
mov    0x18(%rsp),%rsi
call   *0xa3b28(%rip)        # ADDR <_DYNAMIC+0x280>
ud2
mov    $0x10,%eax
mov    %rax,0x20(%rsp)
mov    %rbp,0x18(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b90>
mov    %rax,%rbx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1bc5>
mov    %rax,%rbx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1bc5>
mov    %rax,%rbx
mov    0x38(%rsp),%r12
lea    0x80(%rsp),%rdi
call   ADDR <core::ptr::drop_glue::<expanse_trie::set::ExpanseSet>>
mov    0x68(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1be6>
mov    %r12,%rdi
call   *0xa3a92(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
call   *0xa3aec(%rip)        # ADDR <_DYNAMIC+0x298>
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
