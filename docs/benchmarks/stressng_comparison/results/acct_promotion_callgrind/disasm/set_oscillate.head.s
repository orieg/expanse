push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x3d8,%rsp
mov    %rdi,%rbx
lea    0xa8(%rsp),%rdi
mov    $0x1f0,%edx
xor    %esi,%esi
call   *0xa5887(%rip)        # ADDR <memset@GLIBC_2.2.5>
xorps  %xmm0,%xmm0
movups %xmm0,0x2c8(%rsp)
movups %xmm0,0x2d8(%rsp)
movups %xmm0,0x2e8(%rsp)
movups %xmm0,0x2f8(%rsp)
movq   $0x0,0x78(%rsp)
movq   $0x0,0x90(%rsp)
movl   $0x3,0xa0(%rsp)
movups %xmm0,0x298(%rsp)
movups %xmm0,0x2a8(%rsp)
movups %xmm0,0x2b1(%rsp)
movq   $0xffffffffffffffff,0x308(%rsp)
movups %xmm0,0x310(%rsp)
movups %xmm0,0x320(%rsp)
movups %xmm0,0x32a(%rsp)
mov    0x8(%rbx),%r14
mov    %rbx,0x60(%rsp)
mov    0x10(%rbx),%rax
test   %rax,%rax
mov    %r14,0x58(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a8c>
lea    0x2c8(%rsp),%r13
lea    (%r14,%rax,8),%rax
mov    %rax,0x350(%rsp)
lea    0xa2d52(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
mov    %rax,0x28(%rsp)
xor    %eax,%eax
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x115>
nopw   0x0(%rax,%rax,1)
xor    %ebx,%ebx
add    $0x8,%r14
movzbl %bl,%eax
mov    0x358(%rsp),%rcx
add    %rax,%rcx
mov    %rcx,%rax
cmp    0x350(%rsp),%r14
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a8e>
mov    %rax,0x358(%rsp)
mov    (%r14),%rax
mov    %rax,0x30(%rsp)
mov    %rax,0x70(%rsp)
lea    0x70(%rsp),%rax
mov    0x70(%rsp),%rbp
cmpl   $0x2,0x78(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x400>
mov    %rbp,%rax
shr    $0x8,%rax
cmp    %rax,0x308(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x830>
mov    0x320(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x4c8>
mov    %ebp,%eax
shr    $0x3,%eax
and    $0x18,%eax
mov    (%rbx,%rax,1),%rcx
bts    %rbp,%rcx
mov    %rcx,(%rbx,%rax,1)
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
incq   0x330(%rsp)
mov    0x338(%rsp),%eax
lea    0x1(%rax),%ecx
mov    %cx,0x338(%rsp)
mov    0x2c8(%rsp),%rcx
mov    %al,0x8(%rcx)
cmpw   $0x100,0x338(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa5>
mov    0x330(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movq   $0x0,0x330(%rsp)
mov    0x318(%rsp),%rax
cmp    $0x2,%rax
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c2>
movzbl 0x311(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x21e>
mov    0x2d0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cd84c(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
movzbl 0x312(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x263>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cd891(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
movzbl 0x313(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x2a8>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cd8d6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
movzbl 0x314(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x2ed>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cd91b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
movzbl 0x315(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x332>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cd960(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
movzbl 0x316(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x377>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cd9a5(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
movzbl 0x317(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3b8>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cd9e6(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b31>
mov    0xa0(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf5c>
addq   $0xffffffffffffffc0,0x298(%rsp)
decq   0x2a0(%rsp)
mov    0xb0(%rsp),%rax
mov    %rax,(%rbx)
mov    %rbx,0xb0(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xfa7>
nopw   0x0(%rax,%rax,1)
mov    0xa0(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
cmpq   $0x0,0x78(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x487>
mov    0x80(%rsp),%rcx
mov    0x88(%rsp),%r15
mov    $0x1,%r12d
test   %rcx,%rcx
mov    %rcx,(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x526>
mov    %rcx,%rbx
cmp    -0x8(%r15,%rcx,8),%rbp
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6ab>
cmp    $0x1f,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6f9>
mov    $0x2,%eax
cmp    $0x1,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x52d>
lea    0x1(%rcx),%r12
cmp    $0x10,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x123a>
cmp    $0x3,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1469>
mov    $0x3,%r12d
mov    $0x1,%sil
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1489>
addq   $0x10,0x298(%rsp)
incq   0x2a0(%rsp)
incq   0x2b0(%rsp)
mov    0xd8(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x573>
mov    (%rbx),%rax
mov    %rax,0xd8(%rsp)
movq   $0x0,(%rbx)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x686>
mov    0x328(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x830>
movzwl 0x338(%rsp),%ecx
cmp    %bpl,-0x1(%rax,%rcx,1)
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x823>
lea    -0x19(%rcx),%edx
cmp    $0xffe9,%dx
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x830>
cmp    $0x10,%cx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12ce>
cmp    $0x2,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x830>
lea    0x4(%rcx),%edx
and    $0x1c,%edx
lea    0x3(%rcx),%esi
and    $0x3c,%esi
cmp    %esi,%edx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x830>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1803>
mov    $0x1,%eax
xor    %ebx,%ebx
mov    $0x1,%sil
mov    %rax,%rcx
mov    (%rsp),%rdx
cmp    %rdx,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1494>
lea    (%r15,%rbx,8),%rsi
lea    (%r15,%rbx,8),%rdi
add    $0x8,%rdi
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *0xa5337(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,(%r15,%rbx,8)
movq   $0x1,0x78(%rsp)
mov    %r12,0x80(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x10(%rsp)
mov    $0x1000,%eax
mov    %rax,0x8(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa52f5(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b4d>
mov    0x360(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b4d>
mov    $0x1000,%edx
mov    %rbx,%rdi
xor    %esi,%esi
call   *0xa52dc(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2b8(%rsp),%rax
mov    %rax,(%rbx)
movq   $0x6,0x8(%rbx)
mov    %rbx,0x2b8(%rsp)
lea    0xff0(%rbx),%rax
mov    $0xffffffffffffff08,%rcx
lea    0x2c8(%rsp),%r13
data16 cs nopw 0x0(%rax,%rax,1)
mov    0xd8(%rsp),%rdx
mov    %rdx,(%rax)
mov    %rax,0xd8(%rsp)
lea    -0x10(%rax),%rdx
mov    0xd8(%rsp),%rsi
mov    %rsi,-0x10(%rax)
mov    %rdx,0xd8(%rsp)
lea    -0x20(%rax),%rdx
mov    0xd8(%rsp),%rsi
mov    %rsi,-0x20(%rax)
mov    %rdx,0xd8(%rsp)
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x67a>
lea    -0x30(%rax),%rdx
mov    0xd8(%rsp),%rsi
mov    %rsi,-0x30(%rax)
mov    %rdx,0xd8(%rsp)
add    $0xffffffffffffffc0,%rax
add    $0x4,%rcx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x610>
movq   $0x0,0x40(%rbx)
add    $0x40,%rbx
mov    %rbp,(%rbx)
movq   $0x1,0x78(%rsp)
movq   $0x1,0x80(%rsp)
mov    %rbx,0x88(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
mov    (%rsp),%rax
xor    %ebx,%ebx
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6dd>
nopl   (%rax)
mov    %rbx,%rcx
mov    %rax,%rdx
shr    $1,%rdx
add    %rdx,%rbx
cmp    %rbp,(%r15,%rbx,8)
cmova  %rcx,%rbx
sub    %rdx,%rax
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6c0>
cmp    %rbp,(%r15,%rbx,8)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
adc    $0x0,%rbx
mov    (%rsp),%rcx
cmp    $0x1f,%rcx
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x452>
mov    %r15,%r12
xorps  %xmm0,%xmm0
movaps %xmm0,0x40(%rsp)
lea    0x0(,%rcx,8),%rbx
xor    %r15d,%r15d
nop
mov    (%r12,%r15,1),%rdx
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
lea    0x98(%rsp),%rdi
lea    0x40(%rsp),%rsi
mov    $0x8,%ecx
lea    0x360(%rsp),%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
add    $0x8,%r15
cmp    %r15,%rbx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x710>
lea    0x98(%rsp),%rdi
lea    0x40(%rsp),%rsi
mov    %rbp,%rdx
mov    $0x8,%ecx
mov    %r13,%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
movaps 0x40(%rsp),%xmm0
movaps %xmm0,0x360(%rsp)
mov    $0x100,%eax
cmpq   $0x21,(%rsp)
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x7cb>
add    $0x18,%rbx
mov    %rbx,%rax
and    $0xffffffffffffffe0,%rax
cmp    $0x177,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1270>
lea    -0x2cbcf9(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rax,%rcx,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x121a>
sub    %rax,0x298(%rsp)
decq   0x2a0(%rsp)
cmp    $0x3e,%r13b
mov    (%rsp),%rbx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1aec>
mov    0xa8(%rsp,%r13,8),%rax
mov    %rax,(%r12)
mov    %r12,0xa8(%rsp,%r13,8)
lea    0x2c8(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12a2>
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
nopl   0x0(%rax)
mov    0x318(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa76>
mov    0x330(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa50>
movq   $0x0,0x330(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa50>
movzbl 0x311(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8a3>
mov    0x2d0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cded1(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa50>
movzbl 0x312(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8e8>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cdf16(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa50>
movzbl 0x313(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x92d>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cdf5b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa50>
movzbl 0x314(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x972>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cdfa0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa50>
movzbl 0x315(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x9b7>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2cdfe5(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa50>
movzbl 0x316(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x9fc>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ce02a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa50>
movzbl 0x317(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa3d>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ce06b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b31>
nopw   0x0(%rax,%rax,1)
movq   $0xffffffffffffffff,0x308(%rsp)
movw   $0x0,0x338(%rsp)
xorps  %xmm0,%xmm0
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
mov    0xa0(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xab0>
lea    0x98(%rsp),%rdi
lea    0x80(%rsp),%rsi
mov    %rbp,%rdx
mov    $0x8,%ecx
mov    %r13,%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
test   %al,%al
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
incq   0x90(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
nop
lea    0x78(%rsp),%rdi
mov    %rbp,%rsi
call   *0xa523a(%rip)        # ADDR <_DYNAMIC+0x750>
mov    0x30(%rsp),%rax
mov    %rax,0x40(%rsp)
lea    0x40(%rsp),%rax
mov    0xa0(%rsp),%eax
mov    0x40(%rsp),%rdx
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd90>
mov    0x318(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd26>
mov    0x330(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd00>
movq   $0x0,0x330(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd00>
movzbl 0x311(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb54>
mov    0x2d0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2ce182(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd00>
movzbl 0x312(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb99>
mov    0x2d8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2ce1c7(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd00>
movzbl 0x313(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xbde>
mov    0x2e0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2ce20c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd00>
movzbl 0x314(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xc23>
mov    0x2e8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2ce251(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd00>
movzbl 0x315(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xc68>
mov    0x2f0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2ce296(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd00>
movzbl 0x316(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xcad>
mov    0x2f8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2ce2db(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd00>
movzbl 0x317(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xcee>
mov    0x300(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2ce31c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b31>
nopl   0x0(%rax,%rax,1)
movq   $0xffffffffffffffff,0x308(%rsp)
movw   $0x0,0x338(%rsp)
xorps  %xmm0,%xmm0
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
mov    0x78(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
cmp    $0x1,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xdb0>
mov    0x80(%rsp),%r15
test   %r15,%r15
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
mov    0x88(%rsp),%rbx
cmp    $0x1,%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe12>
cmp    %rdx,(%rbx)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
addq   $0xfffffffffffffff0,0x298(%rsp)
decq   0x2a0(%rsp)
mov    0xd8(%rsp),%rax
mov    %rax,(%rbx)
mov    %rbx,0xd8(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe02>
nopl   0x0(%rax,%rax,1)
lea    0x78(%rsp),%rdi
mov    %rdx,%rsi
call   *0xa4f62(%rip)        # ADDR <_DYNAMIC+0x758>
mov    %eax,%ebx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
data16 cs nopw 0x0(%rax,%rax,1)
lea    0x98(%rsp),%rdi
lea    0x80(%rsp),%rsi
mov    $0x8,%ecx
call   ADDR <expanse_trie::mutate::remove::<false, false>>
test   %al,%al
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
mov    0x90(%rsp),%rax
dec    %rax
mov    %rax,0x90(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe02>
mov    $0x1,%bl
cmp    $0x1e,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
lea    0x78(%rsp),%rdi
call   ADDR <<expanse_trie::set::ExpanseSet>::condense_to_root_leaf::<false>>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
movq   $0x0,0x78(%rsp)
mov    $0x1,%bl
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
mov    %r15,%rax
xor    %r12d,%r12d
nopl   0x0(%rax,%rax,1)
mov    %r12,%rcx
mov    %rax,%rsi
shr    $1,%rsi
add    %rsi,%r12
cmp    %rdx,(%rbx,%r12,8)
cmova  %rcx,%r12
sub    %rsi,%rax
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe20>
cmp    %rdx,(%rbx,%r12,8)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
lea    (%rbx,%r12,8),%rdi
lea    -0x1(%r15),%rax
mov    %rax,0x30(%rsp)
cmp    $0x4,%r15
setb   %cl
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xe86>
mov    $0x8,%edx
cmp    $0x3,%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12ff>
mov    $0x10,%edx
mov    $0xe,%al
mov    $0xe,%r13d
xor    %ecx,%ecx
mov    $0x10,%r8d
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1326>
cmp    $0x12,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11fc>
mov    $0x13,%r8d
cmp    $0x11,%r15
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xeb1>
lea    0x2(%r15),%r8
lea    0x3(%r15),%rax
xor    %r8,%rax
cmp    $0x4,%rax
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1877>
and    $0x3c,%r8d
shl    $0x3,%r8
cmp    $0x177,%r8
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18c5>
mov    %r8,%rdx
lea    -0x2cc3f7(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rdx,%rax,1),%r13d
cmp    $0xff,%r13
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1323>
lock add %r8,0x298(%rsp)
lock incq 0x2a0(%rsp)
lock incq 0x2b0(%rsp)
cmp    $0xf,%rdx
mov    %ecx,(%rsp)
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1461>
movq   $0x0,0x360(%rsp)
mov    $0x10,%eax
mov    %rax,0x10(%rsp)
mov    $0x10,%esi
lea    0x360(%rsp),%rdi
mov    %rdx,%r13
call   *0xa496a(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b48>
mov    0x360(%rsp),%rbp
test   %rbp,%rbp
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b48>
mov    %rbp,%rdi
xor    %esi,%esi
mov    %r13,%rdx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1456>
mov    0x98(%rsp),%r15
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a81>
shl    $0x6,%rax
lock subq $0x40,(%r15,%rax,1)
lock decq 0x8(%r15,%rax,1)
mov    0x1000(%r15),%rdi
add    $0x40,%rdi
mov    $0x40,%edx
mov    $0x40,%ecx
mov    %rbx,%rsi
call   *0xa4b29(%rip)        # ADDR <_DYNAMIC+0x528>
mov    0x2c8(%rsp),%rax
xorps  %xmm0,%xmm0
movups %xmm0,(%rax)
movb   $0x7f,0xf(%rax)
movb   $0xff,0x8(%rax)
mov    0x318(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa5>
mov    0x330(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d4>
movq   $0x0,0x330(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d4>
movzbl 0x311(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1030>
mov    0x2d0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ce65e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d4>
movzbl 0x312(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1075>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ce6a3(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d4>
movzbl 0x313(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x10ba>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ce6e8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d4>
movzbl 0x314(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x10ff>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ce72d(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d4>
movzbl 0x315(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1144>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ce772(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d4>
movzbl 0x316(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1189>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ce7b7(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11d4>
movzbl 0x317(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11ca>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2ce7f8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b31>
movq   $0xffffffffffffffff,0x308(%rsp)
movw   $0x0,0x338(%rsp)
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xaa5>
cmp    $0x1a,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12e6>
mov    $0xc0,%edx
cmp    $0x19,%r15
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12ff>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1877>
lock sub %rax,0x298(%rsp)
lock decq 0x2a0(%rsp)
lea    0x2c8(%rsp),%r13
mov    (%rsp),%rbx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1299>
cmp    $0x18,%rcx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17cc>
cmp    $0x10,%rcx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17e7>
mov    $0x18,%eax
mov    $0x11,%r12d
mov    (%rsp),%rcx
add    $0x3,%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x147d>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17e7>
lock sub %rax,0x298(%rsp)
lock decq 0x2a0(%rsp)
movabs $0x7ffffffffffffff0,%rcx
cmp    %rcx,%rax
mov    (%rsp),%rbx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ac5>
mov    %r12,%rdi
call   *0xa4536(%rip)        # ADDR <free@GLIBC_2.2.5>
inc    %rbx
mov    %rbx,0x90(%rsp)
movaps 0x360(%rsp),%xmm0
lea    0x80(%rsp),%rax
movups %xmm0,(%rax)
movq   $0x2,0x78(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
cmp    $0x10,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17f0>
cmp    $0x18,%ecx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x830>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1803>
cmp    $0x22,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1866>
mov    $0x100,%edx
cmp    $0x21,%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1877>
lea    0x8(%rdx),%r8d
and    $0x1d0,%r8d
lea    -0x2cc838(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rdx,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xee2>
mov    %r13d,%eax
add    %r8,0x298(%rsp)
incq   0x2a0(%rsp)
incq   0x2b0(%rsp)
cmp    $0x3e,%al
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1afa>
mov    0xa8(%rsp,%r13,8),%rbp
test   %rbp,%rbp
mov    %ecx,(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1367>
mov    0x0(%rbp),%rax
mov    %rax,0xa8(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1451>
cmp    $0x100,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1461>
mov    %rdx,0x38(%rsp)
mov    %r8,0x18(%rsp)
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x10(%rsp)
mov    $0x1000,%eax
mov    %rax,0x8(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa44ea(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b4d>
mov    0x360(%rsp),%rbp
test   %rbp,%rbp
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b4d>
mov    $0x1000,%edx
mov    %rbp,%rdi
xor    %esi,%esi
call   *0xa44d1(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2b8(%rsp),%rax
mov    %rax,0x0(%rbp)
mov    %r13,0x8(%rbp)
mov    %rbp,0x2b8(%rsp)
mov    0x18(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b22>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %di
movzwl %ax,%eax
mov    0x38(%rsp),%rdx
cs nopw 0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %rdi,%rcx
lea    (%rcx,%rbp,1),%r8
add    $0x40,%r8
mov    0xa8(%rsp,%r13,8),%rsi
mov    %rsi,0x40(%rbp,%rcx,1)
mov    %r8,0xa8(%rsp,%r13,8)
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1420>
add    $0x40,%rbp
mov    %rbp,%rdi
xor    %esi,%esi
call   *0xa4454(%rip)        # ADDR <memset@GLIBC_2.2.5>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1917>
mov    %rdx,%r8
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18fa>
lea    0x4(%rcx),%eax
and    $0x1c,%eax
add    $0x3,%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17e7>
cmp    $0x11,%r12
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1841>
xor    %esi,%esi
lea    0x3(%r12),%ecx
and    $0x3c,%ecx
mov    %r12,%rax
mov    %r15,0x18(%rsp)
lea    0x0(,%rcx,8),%edx
lea    0x8(,%rcx,8),%ecx
movabs $0x7ffffffffffffff0,%r15
and    %ecx,%r15d
mov    %rax,%r12
lea    -0x2cc9e5(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rdx,%rax,1),%r13d
cmp    $0xff,%r13
mov    %r12,0x348(%rsp)
mov    %esi,0x38(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x151d>
add    %r15,0x298(%rsp)
incq   0x2a0(%rsp)
incq   0x2b0(%rsp)
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1afa>
mov    0xa8(%rsp,%r13,8),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1568>
mov    (%r12),%rax
mov    %rax,0xa8(%rsp,%r13,8)
mov    0x18(%rsp),%r15
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x168d>
lock add %r15,0x298(%rsp)
lock incq 0x2a0(%rsp)
lock incq 0x2b0(%rsp)
cmp    $0xf,%edx
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1640>
mov    $0x1,%esi
mov    %rdx,%rdi
mov    %rdx,%r13
call   *0xa4356(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r12
test   %rax,%rax
mov    0x18(%rsp),%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1698>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b3e>
mov    %rdx,0x340(%rsp)
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x10(%rsp)
mov    $0x1000,%eax
mov    %rax,0x8(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa42f8(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b4d>
mov    0x360(%rsp),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b4d>
mov    $0x1000,%edx
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa42df(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2b8(%rsp),%rax
mov    %rax,(%r12)
mov    %r13,0x8(%r12)
mov    %r12,0x2b8(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r15w
movzwl %ax,%eax
nopw   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %r15,%rcx
lea    (%r12,%rcx,1),%rdx
add    $0x40,%rdx
mov    0xa8(%rsp,%r13,8),%rsi
mov    %rsi,0x40(%r12,%rcx,1)
mov    %rdx,0xa8(%rsp,%r13,8)
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1600>
add    $0x40,%r12
mov    0x18(%rsp),%r15
mov    0x340(%rsp),%rdx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x168d>
movq   $0x0,0x360(%rsp)
mov    $0x10,%eax
mov    %rax,0x10(%rsp)
mov    $0x10,%esi
lea    0x360(%rsp),%rdi
mov    %rdx,%r13
call   *0xa4234(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b48>
mov    0x360(%rsp),%r12
test   %r12,%r12
mov    0x18(%rsp),%r15
mov    %r13,%rdx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b1b>
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa4218(%rip)        # ADDR <memset@GLIBC_2.2.5>
lea    0x0(,%rbx,8),%rdx
mov    %r12,%rdi
mov    %r15,%rsi
mov    0xa4113(%rip),%rax        # ADDR <memcpy@GLIBC_2.14>
mov    %r15,%r13
mov    %rax,%r15
call   *%rax
mov    %rbp,(%r12,%rbx,8)
lea    (%r12,%rbx,8),%rdi
add    $0x8,%rdi
mov    %r13,%rbp
lea    0x0(,%rbx,8),%rsi
add    %r13,%rsi
mov    (%rsp),%r13
mov    %r13,%rdx
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *%r15
cmpb   $0x0,0x38(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x16ec>
mov    %rbp,%rdi
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x16fd>
cmp    $0x11,%r13
mov    %rbp,%rdi
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x175b>
add    $0x3,%r13d
and    $0x3c,%r13d
lea    0x8(,%r13,8),%rax
shl    $0x3,%r13
movabs $0x7ffffffffffffff0,%rcx
and    %rcx,%rax
lea    -0x2ccc44(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl 0x0(%r13,%rcx,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1786>
sub    %rax,0x298(%rsp)
decq   0x2a0(%rsp)
cmp    $0x3d,%r13b
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1aec>
mov    0xa8(%rsp,%r13,8),%rax
mov    %rax,(%rdi)
mov    %rdi,0xa8(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x179e>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
shl    $0x6,%eax
add    $0xc0,%rax
mov    %rax,%r13
lea    -0x2ccc9e(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl 0x0(%r13,%rcx,1),%r13d
cmp    $0xff,%r13
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x172c>
lock sub %rax,0x298(%rsp)
lock decq 0x2a0(%rsp)
call   *0xa403a(%rip)        # ADDR <free@GLIBC_2.2.5>
movq   $0x1,0x78(%rsp)
mov    0x348(%rsp),%rax
mov    %rax,0x80(%rsp)
mov    %r12,0x88(%rsp)
lea    0x2c8(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
setne  %al
movzbl %al,%eax
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
cmp    %eax,%ecx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x147d>
mov    (%rsp),%rdx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x540>
mov    $0x18,%edx
lea    0x3(%rcx),%esi
and    $0x3c,%esi
cmp    %esi,%edx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x830>
mov    %bpl,(%rax,%rcx,1)
mov    0x2c8(%rsp),%rax
mov    0x8(%rax),%rdx
mov    $0xffffffffffffff00,%rsi
and    %rsi,%rdx
or     %rcx,%rdx
mov    %rdx,0x8(%rax)
incw   0x338(%rsp)
incq   0x330(%rsp)
incq   0x90(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xabe>
mov    %r15,0x18(%rsp)
xor    %r15d,%r15d
cmp    $0x19,%r12
setae  %r15b
shl    $0x6,%r15d
add    $0xc0,%r15
xor    %esi,%esi
mov    %r15,%rdx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14b7>
lea    0x2(%r15),%r8
lea    0x3(%r15),%rax
xor    %r8,%rax
cmp    $0x4,%rax
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18b0>
lea    0x8(%rdi),%rsi
mov    0x30(%rsp),%r15
mov    %r15,%rdx
sub    %r12,%rdx
shl    $0x3,%rdx
call   *0xa4000(%rip)        # ADDR <memmove@GLIBC_2.2.5>
movq   $0x1,0x78(%rsp)
mov    %r15,0x80(%rsp)
mov    %rbx,0x88(%rsp)
mov    $0x1,%bl
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
and    $0xfffffffffffffffc,%r8
shl    $0x3,%r8
cmp    $0x177,%r8
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xec6>
lock add %r8,0x298(%rsp)
lock incq 0x2a0(%rsp)
lock incq 0x2b0(%rsp)
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r8
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ac5>
movl   $0x0,(%rsp)
mov    $0x1,%esi
mov    %r8,%rdi
mov    %r8,%r13
call   *0xa3f9d(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%rbp
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b3e>
lea    0x0(,%r12,8),%rdx
mov    %rbp,%rdi
mov    %rbx,%rsi
mov    0xa3e94(%rip),%r13        # ADDR <memcpy@GLIBC_2.14>
call   *%r13
lea    0x0(,%r12,8),%rdi
add    %rbp,%rdi
lea    (%rbx,%r12,8),%rsi
add    $0x8,%rsi
lea    -0x1(%r15),%rdx
sub    %r12,%rdx
shl    $0x3,%rdx
call   *%r13
cmpb   $0x0,(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1960>
lea    0x2c8(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1976>
cmp    $0x11,%r15
lea    0x2c8(%rsp),%r13
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19bc>
add    $0x3,%r15d
and    $0x3c,%r15d
lea    0x8(,%r15,8),%rax
mov    %r15,%rcx
shl    $0x3,%rcx
and    $0xfffffffffffffff0,%rax
cmp    $0x177,%rcx
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19e0>
lock sub %rax,0x298(%rsp)
lock decq 0x2a0(%rsp)
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%rcx
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a46>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1ac5>
mov    $0xc0,%eax
mov    $0xc0,%ecx
cmp    $0x19,%r15
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19e0>
mov    $0x100,%eax
mov    $0x100,%ecx
cmp    $0x21,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a74>
lea    -0x2ccf0e(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rdx,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a2c>
sub    %rax,0x298(%rsp)
decq   0x2a0(%rsp)
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1aec>
mov    0xa8(%rsp,%r13,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0xa8(%rsp,%r13,8)
lea    0x2c8(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1a4f>
lock sub %rax,0x298(%rsp)
lock decq 0x2a0(%rsp)
lea    0x2c8(%rsp),%r13
mov    %rbx,%rdi
call   *0xa3d89(%rip)        # ADDR <free@GLIBC_2.2.5>
movq   $0x1,0x78(%rsp)
mov    0x30(%rsp),%rax
mov    %rax,0x80(%rsp)
mov    %rbp,0x88(%rsp)
mov    $0x1,%bl
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
add    $0x3,%r15
and    $0xfffffffffffffffc,%r15
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1976>
call   *0xa4051(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf79>
xor    %eax,%eax
mov    %rax,0x68(%rsp)
lea    0x68(%rsp),%rax
mov    0x60(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1aae>
mov    0x58(%rsp),%rdi
call   *0xa3d2a(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x68(%rsp),%rax
add    $0x3d8,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
lea    -0x2cf1a7(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0xa10ad(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x9a236(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x27(%rsp),%rdx
mov    $0x11,%esi
call   *0xa3d86(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b5d>
mov    $0x3e,%esi
lea    0x9a200(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b06>
mov    $0x3e,%esi
lea    0x9a222(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    %rax,0x28(%rsp)
mov    %r13,%rdi
mov    0x28(%rsp),%rdx
call   *0xa3c97(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b5d>
mov    %rdx,0x8(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b4d>
lea    0x9a1ff(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
call   *0xa3e91(%rip)        # ADDR <_DYNAMIC+0x418>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b5d>
mov    $0x8,%r13d
mov    $0x8,%esi
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b0b>
mov    $0x10,%eax
mov    %rax,0x10(%rsp)
mov    %r13,0x8(%rsp)
mov    0x10(%rsp),%rdi
mov    0x8(%rsp),%rsi
call   *0xa3ccb(%rip)        # ADDR <_DYNAMIC+0x280>
ud2
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b63>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b63>
mov    %rax,%rbx
lea    0x78(%rsp),%rdi
call   ADDR <core::ptr::drop_glue::<expanse_trie::set::ExpanseSet>>
mov    0x60(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1b86>
mov    0x58(%rsp),%rdi
call   *0xa3c52(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
call   *0xa3cac(%rip)        # ADDR <_DYNAMIC+0x298>
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
