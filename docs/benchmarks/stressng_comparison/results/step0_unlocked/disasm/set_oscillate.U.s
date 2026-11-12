push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x3d8,%rsp
mov    %rdi,%rbx
lea    0xa8(%rsp),%rdi
xor    %r14d,%r14d
mov    $0x1f0,%edx
xor    %esi,%esi
call   *0xa44b4(%rip)        # ADDR <memset@GLIBC_2.2.5>
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
mov    0x8(%rbx),%rcx
mov    %rbx,0x60(%rsp)
mov    0x10(%rbx),%rax
test   %rax,%rax
mov    %rcx,0x28(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18b8>
lea    0x2c8(%rsp),%r13
lea    (%rcx,%rax,8),%rax
mov    %rax,0x348(%rsp)
lea    0xa197f(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
mov    %rax,0x18(%rsp)
xor    %r14d,%r14d
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11a>
nopl   0x0(%rax,%rax,1)
xor    %ebx,%ebx
mov    0x350(%rsp),%rcx
add    $0x8,%rcx
movzbl %bl,%eax
mov    0x358(%rsp),%r14
add    %rax,%r14
cmp    0x348(%rsp),%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18b8>
mov    %r14,0x358(%rsp)
mov    %rcx,0x350(%rsp)
mov    (%rcx),%rax
mov    %rax,0x30(%rsp)
mov    %rax,0x70(%rsp)
lea    0x70(%rsp),%rax
mov    0x70(%rsp),%rbp
cmpl   $0x2,0x78(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x410>
mov    %rbp,%rax
shr    $0x8,%rax
cmp    %rax,0x308(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6d0>
mov    0x320(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x4d3>
mov    %ebp,%eax
shr    $0x3,%eax
and    $0x18,%eax
mov    (%rbx,%rax,1),%rcx
bts    %rbp,%rcx
mov    %rcx,(%rbx,%rax,1)
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
incq   0x330(%rsp)
mov    0x338(%rsp),%eax
lea    0x1(%rax),%ecx
mov    %cx,0x338(%rsp)
mov    0x2c8(%rsp),%rcx
mov    %al,0x8(%rcx)
cmpw   $0x100,0x338(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x945>
mov    0x330(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3cf>
movq   $0x0,0x330(%rsp)
mov    0x318(%rsp),%rax
cmp    $0x2,%rax
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3cf>
movzbl 0x311(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x22b>
mov    0x2d0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c23a9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3cf>
movzbl 0x312(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x270>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c23ee(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3cf>
movzbl 0x313(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x2b5>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c2433(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3cf>
movzbl 0x314(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x2fa>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c2478(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3cf>
movzbl 0x315(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x33f>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c24bd(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3cf>
movzbl 0x316(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x384>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c2502(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3cf>
movzbl 0x317(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x3c5>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c2543(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1974>
mov    0xa0(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xefd>
addq   $0xffffffffffffffc0,0x298(%rsp)
decq   0x2a0(%rsp)
mov    0xb0(%rsp),%rax
mov    %rax,(%rbx)
mov    %rbx,0xb0(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf48>
nopw   0x0(%rax,%rax,1)
mov    0xa0(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x950>
cmpq   $0x0,0x78(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x492>
mov    0x80(%rsp),%rdx
mov    0x88(%rsp),%r15
mov    $0x1,%r12d
test   %rdx,%rdx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x531>
mov    %rdx,%rbx
cmp    -0x8(%r15,%rdx,8),%rbp
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6ab>
cmp    $0x1f,%rdx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd85>
mov    $0x2,%esi
cmp    $0x1,%rdx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x538>
lea    0x1(%rdx),%rsi
cmp    $0x10,%rdx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11b8>
cmp    $0x3,%rdx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x120e>
mov    $0x3,%esi
mov    $0x1,%dil
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1226>
addq   $0x10,0x298(%rsp)
incq   0x2a0(%rsp)
incq   0x2b0(%rsp)
mov    0xd8(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x57a>
mov    (%rbx),%rax
mov    %rax,0xd8(%rsp)
movq   $0x0,(%rbx)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x686>
mov    0x328(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6d0>
movzwl 0x338(%rsp),%ecx
cmp    %bpl,-0x1(%rax,%rcx,1)
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6c2>
lea    -0x19(%rcx),%edx
cmp    $0xffe9,%dx
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6d0>
cmp    $0x10,%cx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11de>
cmp    $0x2,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6d0>
lea    0x4(%rcx),%edx
and    $0x1c,%edx
lea    0x3(%rcx),%esi
and    $0x3c,%esi
cmp    %esi,%edx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6d0>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1263>
mov    $0x1,%esi
xor    %ebx,%ebx
mov    $0x1,%dil
mov    %rsi,%rax
cmp    %rdx,%rsi
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12b4>
lea    (%r15,%rbx,8),%rsi
lea    (%r15,%rbx,8),%rdi
add    $0x8,%rdi
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *0xa3f60(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbp,(%r15,%rbx,8)
movq   $0x1,0x78(%rsp)
mov    %r12,0x80(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x8(%rsp)
mov    $0x1000,%eax
mov    %rax,(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa3f1f(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1963>
mov    0x360(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1963>
mov    $0x1000,%edx
mov    %rbx,%rdi
xor    %esi,%esi
call   *0xa3f06(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2b8(%rsp),%rax
mov    %rax,(%rbx)
movq   $0x6,0x8(%rbx)
mov    %rbx,0x2b8(%rsp)
lea    0xff0(%rbx),%rax
mov    $0xffffffffffffff08,%rcx
lea    0x2c8(%rsp),%r13
nopl   0x0(%rax,%rax,1)
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
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
cmp    $0x1,%rdx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd45>
xor    %ebx,%ebx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd6d>
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
nopl   0x0(%rax,%rax,1)
mov    0x318(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x916>
mov    0x330(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8f0>
movq   $0x0,0x330(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8f0>
movzbl 0x311(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x743>
mov    0x2d0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c28c1(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8f0>
movzbl 0x312(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x788>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c2906(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8f0>
movzbl 0x313(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x7cd>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c294b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8f0>
movzbl 0x314(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x812>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c2990(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8f0>
movzbl 0x315(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x857>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c29d5(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8f0>
movzbl 0x316(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x89c>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c2a1a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8f0>
movzbl 0x317(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x8dd>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c2a5b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1974>
nopw   0x0(%rax,%rax,1)
movq   $0xffffffffffffffff,0x308(%rsp)
movw   $0x0,0x338(%rsp)
xorps  %xmm0,%xmm0
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
mov    0xa0(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x950>
lea    0x98(%rsp),%rdi
lea    0x80(%rsp),%rsi
mov    %rbp,%rdx
mov    $0x8,%ecx
mov    %r13,%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
test   %al,%al
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
incq   0x90(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
nop
lea    0x78(%rsp),%rdi
mov    %rbp,%rsi
call   *0xa3fca(%rip)        # ADDR <_DYNAMIC+0x750>
mov    0x30(%rsp),%rax
mov    %rax,0x40(%rsp)
lea    0x40(%rsp),%rax
mov    0xa0(%rsp),%eax
mov    0x40(%rsp),%rdx
test   %eax,%eax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xc30>
mov    0x318(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xbc6>
mov    0x330(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xba0>
movq   $0x0,0x330(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xba0>
movzbl 0x311(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x9f4>
mov    0x2d0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c2b72(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xba0>
movzbl 0x312(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa39>
mov    0x2d8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c2bb7(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xba0>
movzbl 0x313(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xa7e>
mov    0x2e0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c2bfc(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xba0>
movzbl 0x314(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xac3>
mov    0x2e8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c2c41(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xba0>
movzbl 0x315(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb08>
mov    0x2f0(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c2c86(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xba0>
movzbl 0x316(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb4d>
mov    0x2f8(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c2ccb(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xba0>
movzbl 0x317(%rsp),%esi
cmp    $0x7,%rsi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xb8e>
mov    0x300(%rsp),%rdi
mov    0x8(%rdi),%r8
lea    -0x2c2d0c(%rip),%r9        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r9,%rsi,8),%rsi
mov    %rsi,%r9
and    %r8,%r9
add    %rcx,%r9
xor    %r8,%r9
and    %rsi,%r9
xor    %r8,%r9
mov    %r9,0x8(%rdi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1974>
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
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xc50>
mov    0x80(%rsp),%r15
test   %r15,%r15
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
mov    0x88(%rsp),%rbx
cmp    $0x1,%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xcb2>
cmp    %rdx,(%rbx)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
addq   $0xfffffffffffffff0,0x298(%rsp)
decq   0x2a0(%rsp)
mov    0xd8(%rsp),%rax
mov    %rax,(%rbx)
mov    %rbx,0xd8(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xca2>
nopl   0x0(%rax,%rax,1)
lea    0x78(%rsp),%rdi
mov    %rdx,%rsi
call   *0xa3cf2(%rip)        # ADDR <_DYNAMIC+0x758>
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
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xca2>
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
xor    %ebp,%ebp
nopw   0x0(%rax,%rax,1)
mov    %rbp,%rcx
mov    %rax,%rsi
shr    $1,%rsi
add    %rsi,%rbp
cmp    %rdx,(%rbx,%rbp,8)
cmova  %rcx,%rbp
sub    %rsi,%rax
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xcc0>
cmp    %rdx,(%rbx,%rbp,8)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf0>
lea    (%rbx,%rbp,8),%rdi
lea    -0x1(%r15),%rax
mov    %rax,0x30(%rsp)
cmp    $0x4,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd11>
xor    %ecx,%ecx
cmp    $0x3,%r15
setne  %cl
mov    $0x2,%r14d
sub    %rcx,%r14
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x15b3>
cmp    $0x12,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x119d>
mov    $0x13,%r14d
cmp    $0x11,%r15
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd3c>
lea    0x2(%r15),%r14
lea    0x3(%r15),%rax
xor    %r14,%rax
cmp    $0x4,%rax
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1574>
and    $0x3c,%r14d
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x15b1>
mov    %rdx,%rax
xor    %ebx,%ebx
nopw   0x0(%rax,%rax,1)
mov    %rbx,%rcx
mov    %rax,%rsi
shr    $1,%rsi
add    %rsi,%rbx
cmp    %rbp,(%r15,%rbx,8)
cmova  %rcx,%rbx
sub    %rsi,%rax
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xd50>
cmp    %rbp,(%r15,%rbx,8)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
adc    $0x0,%rbx
cmp    $0x1f,%rdx
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x45e>
xorps  %xmm0,%xmm0
movaps %xmm0,0x40(%rsp)
mov    %rdx,0x20(%rsp)
lea    0x0(,%rdx,8),%rbx
xor    %r14d,%r14d
lea    0x3a8(%rsp),%r12
data16 cs nopw 0x0(%rax,%rax,1)
mov    (%r15,%r14,1),%rdx
xorps  %xmm0,%xmm0
movaps %xmm0,0x390(%rsp)
movaps %xmm0,0x380(%rsp)
movaps %xmm0,0x370(%rsp)
movaps %xmm0,0x360(%rsp)
movq   $0xffffffffffffffff,0x3a0(%rsp)
movups %xmm0,0x1a(%r12)
movups %xmm0,0x10(%r12)
movups %xmm0,(%r12)
lea    0x98(%rsp),%rdi
lea    0x40(%rsp),%rsi
mov    $0x8,%ecx
lea    0x360(%rsp),%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
add    $0x8,%r14
cmp    %r14,%rbx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xdb0>
lea    0x98(%rsp),%rdi
lea    0x40(%rsp),%rsi
mov    %rbp,%rdx
mov    $0x8,%ecx
mov    %r13,%r8
call   ADDR <expanse_trie::mutate::insert_with_path_flat>
mov    0x20(%rsp),%r14
movaps 0x40(%rsp),%xmm0
add    $0x18,%rbx
and    $0xffffffffffffffe0,%rbx
cmp    $0x21,%r14
mov    $0x100,%eax
cmovb  %rax,%rbx
movaps %xmm0,0x360(%rsp)
sub    %rbx,0x298(%rsp)
decq   0x2a0(%rsp)
cmp    $0x177,%rbx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xead>
lea    -0x2c0efa(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rbx,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xec0>
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1916>
mov    0xa8(%rsp,%r13,8),%rax
mov    %rax,(%r15)
mov    %r15,0xa8(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xec9>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%rbx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18ef>
mov    %r15,%rdi
call   *0xa353f(%rip)        # ADDR <free@GLIBC_2.2.5>
lea    0x2c8(%rsp),%r13
inc    %r14
mov    %r14,0x90(%rsp)
movaps 0x360(%rsp),%xmm0
lea    0x80(%rsp),%rax
movups %xmm0,(%rax)
movq   $0x2,0x78(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
mov    0x98(%rsp),%r14
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18ad>
shl    $0x6,%rax
lock subq $0x40,(%r14,%rax,1)
lock decq 0x8(%r14,%rax,1)
mov    0x1000(%r14),%rdi
add    $0x40,%rdi
mov    $0x40,%edx
mov    $0x40,%ecx
mov    %rbx,%rsi
call   *0xa37b8(%rip)        # ADDR <_DYNAMIC+0x528>
mov    0x2c8(%rsp),%rax
xorps  %xmm0,%xmm0
movups %xmm0,(%rax)
movb   $0x7f,0xf(%rax)
movb   $0xff,0x8(%rax)
mov    0x318(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x945>
mov    0x330(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1175>
movq   $0x0,0x330(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1175>
movzbl 0x311(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xfd1>
mov    0x2d0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c314f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x2,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1175>
movzbl 0x312(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1016>
mov    0x2d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c3194(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x3,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1175>
movzbl 0x313(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x105b>
mov    0x2e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c31d9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x4,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1175>
movzbl 0x314(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x10a0>
mov    0x2e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c321e(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x5,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1175>
movzbl 0x315(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x10e5>
mov    0x2f0(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c3263(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x6,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1175>
movzbl 0x316(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x112a>
mov    0x2f8(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c32a8(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x7,%rax
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1175>
movzbl 0x317(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x116b>
mov    0x300(%rsp),%rsi
mov    0x8(%rsi),%rdi
lea    -0x2c32e9(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
cmp    $0x8,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1974>
movq   $0xffffffffffffffff,0x308(%rsp)
movw   $0x0,0x338(%rsp)
movups %xmm0,0x50(%r13)
movq   $0x0,0x60(%r13)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x945>
cmp    $0x1a,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x11ef>
cmp    $0x19,%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1574>
mov    $0x18,%r14d
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x15b1>
cmp    $0x18,%rdx
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1231>
cmp    $0x10,%rdx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1248>
mov    $0x18,%eax
mov    $0x11,%esi
lea    0x3(%rdx),%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x121e>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1248>
cmp    $0x10,%rcx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1250>
cmp    $0x18,%ecx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6d0>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1263>
cmp    $0x22,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1563>
cmp    $0x21,%r15
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1574>
mov    $0x20,%r14d
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x15b1>
lea    0x4(%rdx),%eax
and    $0x1c,%eax
lea    0x3(%rdx),%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1248>
cmp    $0x11,%rsi
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12a1>
xor    %edi,%edi
lea    0x3(%rsi),%eax
and    $0x3c,%eax
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x12b4>
setne  %al
movzbl %al,%eax
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
cmp    %eax,%ecx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x121e>
mov    %rsi,%r12
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x547>
mov    $0x18,%edx
lea    0x3(%rcx),%esi
and    $0x3c,%esi
cmp    %esi,%edx
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x6d0>
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
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
xor    %eax,%eax
cmp    $0x19,%rsi
setae  %al
lea    0x18(,%rax,8),%rax
xor    %edi,%edi
mov    %rdx,0x20(%rsp)
lea    0x8(,%rax,8),%ecx
movabs $0x7ffffffffffffff0,%r14
and    %ecx,%r14d
add    %r14,0x298(%rsp)
incq   0x2a0(%rsp)
incq   0x2b0(%rsp)
lea    0x0(,%rax,8),%edx
lea    -0x2c136d(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rdx,%rcx,1),%r13d
cmp    $0xff,%r13
mov    %rsi,0x58(%rsp)
mov    %edi,0x38(%rsp)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1332>
cmp    $0x3d,%r13b
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1924>
mov    0xa8(%rsp,%r13,8),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x135d>
mov    (%r12),%rax
mov    %rax,0xa8(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1469>
cmp    $0x1,%eax
jbe    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x142b>
mov    $0x1,%esi
mov    %rdx,%rdi
mov    %rdx,%r14
call   *0xa318c(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r12
test   %rax,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1474>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1945>
mov    %rdx,0x340(%rsp)
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x8(%rsp)
mov    $0x1000,%eax
mov    %rax,(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa3134(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1963>
mov    0x360(%rsp),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1963>
mov    $0x1000,%edx
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa311b(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2b8(%rsp),%rax
mov    %rax,(%r12)
mov    %r13,0x8(%r12)
mov    %r12,0x2b8(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r14w
movzwl %ax,%eax
nopl   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %r14,%rcx
lea    (%r12,%rcx,1),%rdx
add    $0x40,%rdx
mov    0xa8(%rsp,%r13,8),%rsi
mov    %rsi,0x40(%r12,%rcx,1)
mov    %rdx,0xa8(%rsp,%r13,8)
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x13f0>
add    $0x40,%r12
mov    0x340(%rsp),%rdx
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1469>
movq   $0x0,0x360(%rsp)
mov    $0x10,%esi
lea    0x360(%rsp),%rdi
mov    %rdx,%r14
call   *0xa3083(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1945>
mov    0x360(%rsp),%r12
test   %r12,%r12
mov    %r14,%rdx
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1955>
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa306c(%rip)        # ADDR <memset@GLIBC_2.2.5>
lea    0x0(,%rbx,8),%rdx
mov    %r12,%rdi
mov    %r15,%rsi
mov    0xa2f67(%rip),%r14        # ADDR <memcpy@GLIBC_2.14>
call   *%r14
mov    %rbp,(%r12,%rbx,8)
lea    (%r12,%rbx,8),%rdi
add    $0x8,%rdi
lea    (%r15,%rbx,8),%rsi
mov    0x20(%rsp),%r13
mov    %r13,%rdx
sub    %rbx,%rdx
shl    $0x3,%rdx
call   *%r14
cmpb   $0x0,0x38(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14d6>
cmp    $0x11,%r13
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14c5>
add    $0x3,%r13d
and    $0x3c,%r13d
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x14d6>
xor    %eax,%eax
cmp    $0x19,%r13
setae  %al
lea    0x18(,%rax,8),%r13
lea    0x8(,%r13,8),%rax
movabs $0x7ffffffffffffff0,%rcx
and    %rcx,%rax
sub    %rax,0x298(%rsp)
decq   0x2a0(%rsp)
lea    -0x2c157c(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rax,%r13,8),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x152f>
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1916>
mov    0xa8(%rsp,%r13,8),%rax
mov    %rax,(%r15)
mov    %r15,0xa8(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1538>
mov    %r15,%rdi
call   *0xa2ed0(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x58(%rsp),%rax
movq   $0x1,0x78(%rsp)
mov    %rax,0x80(%rsp)
mov    %r12,0x88(%rsp)
lea    0x2c8(%rsp),%r13
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x95e>
lea    0x2(%r15),%r14
lea    0x3(%r15),%rax
xor    %r14,%rax
cmp    $0x4,%rax
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x15ad>
lea    0x8(%rdi),%rsi
mov    0x30(%rsp),%r14
mov    %r14,%rdx
sub    %rbp,%rdx
shl    $0x3,%rdx
call   *0xa2f33(%rip)        # ADDR <memmove@GLIBC_2.2.5>
movq   $0x1,0x78(%rsp)
mov    %r14,0x80(%rsp)
mov    %rbx,0x88(%rsp)
mov    $0x1,%bl
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
and    $0xfffffffffffffffc,%r14
xor    %ecx,%ecx
lea    0x8(,%r14,8),%rdx
and    $0xfffffffffffffff0,%rdx
add    %rdx,0x298(%rsp)
shl    $0x3,%r14
incq   0x2a0(%rsp)
incq   0x2b0(%rsp)
cmp    $0x177,%r14
mov    %rcx,0x20(%rsp)
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x162a>
lea    -0x2c166a(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%r14,%rax,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x165c>
cmp    $0x3d,%r13b
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1924>
mov    0xa8(%rsp,%r13,8),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x16a2>
mov    (%r12),%rax
mov    %rax,0xa8(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1781>
movabs $0x7ffffffffffffff0,%rax
cmp    %rax,%r14
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18ef>
mov    $0x1,%esi
mov    %r14,%rdi
call   *0xa2e8d(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r12
test   %rax,%rax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x178f>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1945>
cmp    $0xf,%r14
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x163d>
movq   $0x0,0x360(%rsp)
mov    $0x10,%esi
lea    0x360(%rsp),%rdi
mov    %r14,%rdx
call   *0xa2e4c(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1945>
mov    0x360(%rsp),%r12
test   %r12,%r12
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1781>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1945>
cmp    $0x101,%r14
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x163d>
mov    %rdx,0x38(%rsp)
movq   $0x0,0x360(%rsp)
mov    $0x40,%eax
mov    %rax,0x8(%rsp)
mov    $0x1000,%eax
mov    %rax,(%rsp)
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0x360(%rsp),%rdi
call   *0xa2de9(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1963>
mov    0x360(%rsp),%r12
test   %r12,%r12
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1963>
mov    $0x1000,%edx
mov    %r12,%rdi
xor    %esi,%esi
call   *0xa2dd0(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x2b8(%rsp),%rax
mov    %rax,(%r12)
mov    %r13,0x8(%r12)
mov    %r12,0x2b8(%rsp)
mov    0x38(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1981>
cmp    $0x7e0,%rdi
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x177d>
mov    $0xfc0,%ax
xor    %edx,%edx
div    %di
movzwl %ax,%eax
nopl   0x0(%rax)
dec    %rax
mov    %rax,%rcx
imul   %rdi,%rcx
lea    (%r12,%rcx,1),%rdx
add    $0x40,%rdx
mov    0xa8(%rsp,%r13,8),%rsi
mov    %rsi,0x40(%r12,%rcx,1)
mov    %rdx,0xa8(%rsp,%r13,8)
cmp    $0x1,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1750>
add    $0x40,%r12
mov    %r12,%rdi
xor    %esi,%esi
mov    %r14,%rdx
call   *0xa2d51(%rip)        # ADDR <memset@GLIBC_2.2.5>
lea    0x0(,%rbp,8),%rdx
mov    %r12,%rdi
mov    %rbx,%rsi
mov    0xa2c4c(%rip),%r14        # ADDR <memcpy@GLIBC_2.14>
call   *%r14
lea    (%r12,%rbp,8),%rdi
lea    (%rbx,%rbp,8),%rsi
add    $0x8,%rsi
lea    -0x1(%r15),%rdx
sub    %rbp,%rdx
shl    $0x3,%rdx
call   *%r14
cmpb   $0x0,0x20(%rsp)
jne    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17da>
cmp    $0x11,%r15
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1882>
add    $0x3,%r15d
and    $0x3c,%r15d
mov    %r15,%rax
lea    0x8(,%rax,8),%rcx
shl    $0x3,%rax
and    $0xfffffffffffffff0,%rcx
sub    %rcx,0x298(%rsp)
decq   0x2a0(%rsp)
cmp    $0x177,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1839>
lea    -0x2c1886(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b3>
movzbl (%rax,%rcx,1),%r13d
cmp    $0xff,%r13
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x184c>
cmp    $0x3e,%r13b
jae    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1916>
mov    0xa8(%rsp,%r13,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0xa8(%rsp,%r13,8)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1855>
movabs $0x7ffffffffffffff0,%rcx
cmp    %rcx,%rax
ja     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18ef>
mov    %rbx,%rdi
call   *0xa2bb3(%rip)        # ADDR <free@GLIBC_2.2.5>
lea    0x2c8(%rsp),%r13
movq   $0x1,0x78(%rsp)
mov    0x30(%rsp),%rax
mov    %rax,0x80(%rsp)
mov    %r12,0x88(%rsp)
mov    $0x1,%bl
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf2>
mov    $0x18,%eax
cmp    $0x19,%r15
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17dd>
mov    $0x20,%eax
cmp    $0x21,%r15
jb     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17dd>
add    $0x3,%r15
and    $0xfffffffffffffffc,%r15
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x17da>
call   *0xa2e55(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0xf1a>
mov    %r14,0x68(%rsp)
lea    0x68(%rsp),%rax
mov    0x60(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x18d8>
mov    0x28(%rsp),%rdi
call   *0xa2b30(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x68(%rsp),%rax
add    $0x3d8,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
lea    -0x2c3b21(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0x9feb3(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0x9903c(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x17(%rsp),%rdx
mov    $0x11,%esi
call   *0xa2b8c(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x198e>
mov    $0x3e,%esi
lea    0x99006(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1930>
mov    $0x3e,%esi
lea    0x99028(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    %rax,0x18(%rsp)
mov    %r13,%rdi
mov    0x18(%rsp),%rdx
call   *0xa2a9d(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x198e>
mov    $0x10,%eax
mov    %rax,0x8(%rsp)
mov    %r14,(%rsp)
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1963>
mov    $0x10,%eax
mov    %rax,0x8(%rsp)
mov    %rdx,(%rsp)
mov    0x8(%rsp),%rdi
mov    (%rsp),%rsi
call   *0xa2ae6(%rip)        # ADDR <_DYNAMIC+0x280>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x198e>
mov    $0x8,%r13d
mov    $0x8,%esi
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1935>
lea    0x98fd0(%rip),%rdi        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
call   *0xa2c62(%rip)        # ADDR <_DYNAMIC+0x418>
ud2
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1994>
jmp    ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x1994>
mov    %rax,%rbx
mov    0x28(%rsp),%r14
lea    0x78(%rsp),%rdi
call   ADDR <core::ptr::drop_glue::<expanse_trie::set::ExpanseSet>>
mov    0x60(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::set_oscillate::__iai_callgrind_wrapper_mod::set_oscillate+0x19ba>
mov    %r14,%rdi
call   *0xa2a4e(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
call   *0xa2aa8(%rip)        # ADDR <_DYNAMIC+0x298>
int3
int3
int3
int3
int3
int3
int3
int3
