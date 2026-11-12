push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x428,%rsp
mov    %rdi,%r14
lea    0x188(%rsp),%rdi
xor    %ebx,%ebx
mov    $0x1f0,%edx
xor    %esi,%esi
call   *0xc9145(%rip)        # ADDR <memset@GLIBC_2.2.5>
xorps  %xmm0,%xmm0
movups %xmm0,0x3a8(%rsp)
movups %xmm0,0x3b8(%rsp)
movups %xmm0,0x3c8(%rsp)
movups %xmm0,0x3d8(%rsp)
movq   $0x0,0x158(%rsp)
movq   $0x0,0x170(%rsp)
movl   $0x3,0x180(%rsp)
movups %xmm0,0x378(%rsp)
movups %xmm0,0x388(%rsp)
movups %xmm0,0x391(%rsp)
movq   $0xffffffffffffffff,0x3e8(%rsp)
movups %xmm0,0x3f0(%rsp)
movups %xmm0,0x400(%rsp)
movups %xmm0,0x40a(%rsp)
mov    0x8(%r14),%rdi
mov    0x10(%r14),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    %r14,0x60(%rsp)
lea    (%rdi,%rax,8),%rax
mov    %rax,0x420(%rsp)
lea    0xbf4e2(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    %rax,0x90(%rsp)
mov    %rdi,0x58(%rsp)
mov    %rdi,%r13
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x102>
mov    %r8,(%r12,%rax,8)
nopl   0x0(%rax)
add    $0x8,%r13
cmp    0x420(%rsp),%r13
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a4a>
mov    0x0(%r13),%rax
mov    %rax,0xb0(%rsp)
lea    0xb0(%rsp),%rcx
not    %rax
mov    %rax,0xb8(%rsp)
lea    0xb8(%rsp),%rax
mov    0xb0(%rsp),%rbp
mov    0xb8(%rsp),%r8
mov    0x158(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b0>
cmp    $0x1,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x200>
mov    0x160(%rsp),%r15
mov    0x168(%rsp),%r14
cmp    $0x3,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x290>
lea    (%r15,%r14,8),%r12
test   %r14,%r14
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x4a1>
lea    -0x1(%r14),%rax
mov    %r14,0x38(%rsp)
cmp    -0x8(%r15,%r14,8),%rbp
jbe    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x35c>
cmp    $0x1f,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x6ca>
cmp    $0x1,%r14
mov    0xc8fb1(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x546>
mov    $0x2,%ebx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x4b6>
nop
mov    0x180(%rsp),%eax
mov    %rbp,%r12
mov    %r8,%r15
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x2a6>
addq   $0x10,0x378(%rsp)
incq   0x380(%rsp)
incq   0x390(%rsp)
mov    0x1f8(%rsp),%r14
test   %r14,%r14
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x373>
mov    (%r14),%rax
mov    %rax,0x1f8(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x46e>
nop
mov    %rbp,%rax
shr    $0x8,%rax
cmp    %rax,0x3e8(%rsp)
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12d0>
mov    0x400(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x501>
mov    %ebp,%r14d
shr    $0x5,%r14d
and    $0x7,%r14d
mov    %ebp,%ecx
and    $0x1f,%ecx
mov    (%rbx,%r14,4),%eax
mov    $0x1,%edx
shl    %cl,%edx
bt     %ecx,%eax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x5a9>
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
mov    0x20(%rbx,%r14,8),%rcx
shr    $0x18,%eax
mov    %r8,(%rcx,%rax,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
cmp    $0x11,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x326>
lea    0x3(%r14),%eax
and    $0x3c,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x344>
mov    0x178(%rsp),%rbx
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a18>
shl    $0x6,%rax
lock addq $0x10,(%rbx,%rax,1)
lock incq 0x8(%rbx,%rax,1)
lock incq 0x10(%rbx,%rax,1)
mov    0x1000(%rbx),%rdi
add    $0x40,%rdi
mov    $0xe,%esi
call   *0xc9099(%rip)        # ADDR <_DYNAMIC+0x520>
mov    %rax,%r14
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x46e>
mov    $0x10,%ebx
mov    $0x10,%edi
mov    $0x1,%esi
call   *0xc8e58(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r14
mov    $0x10,%ebp
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x475>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    $0x18,%eax
cmp    $0x19,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x344>
mov    $0x20,%eax
cmp    $0x21,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x344>
lea    0x3(%r14),%rax
and    $0xfffffffffffffffc,%rax
lea    (%r15,%rax,8),%r12
lea    -0x1(%r14),%rax
mov    %r14,0x38(%rsp)
cmp    -0x8(%r15,%r14,8),%rbp
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18a>
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe8>
cmp    $0x1,%r14
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x681>
xor    %eax,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x6ad>
movq   $0x0,0xc0(%rsp)
mov    $0x40,%ebx
mov    $0x1000,%ebp
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xc0(%rsp),%rdi
call   *0xc8dbf(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    0xc0(%rsp),%r14
test   %r14,%r14
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    $0x1000,%edx
mov    %r14,%rdi
xor    %esi,%esi
call   *0xc8da6(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x398(%rsp),%rax
mov    %rax,(%r14)
movq   $0xe,0x8(%r14)
mov    %r14,0x398(%rsp)
lea    0xff0(%r14),%rax
mov    $0xffffffffffffff08,%rcx
data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    0x1f8(%rsp),%rdx
mov    %rdx,(%rax)
mov    %rax,0x1f8(%rsp)
lea    -0x10(%rax),%rdx
mov    0x1f8(%rsp),%rsi
mov    %rsi,-0x10(%rax)
mov    %rdx,0x1f8(%rsp)
lea    -0x20(%rax),%rdx
mov    0x1f8(%rsp),%rsi
mov    %rsi,-0x20(%rax)
mov    %rdx,0x1f8(%rsp)
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x46a>
lea    -0x30(%rax),%rdx
mov    0x1f8(%rsp),%rsi
mov    %rsi,-0x30(%rax)
mov    %rdx,0x1f8(%rsp)
add    $0xffffffffffffffc0,%rax
add    $0x4,%rcx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x400>
add    $0x40,%r14
xorps  %xmm0,%xmm0
movups %xmm0,(%r14)
mov    %r12,(%r14)
mov    %r15,0x8(%r14)
movq   $0x1,0x158(%rsp)
mov    %r14,0x160(%rsp)
movq   $0x1,0x168(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
mov    $0x1,%ebx
movq   $0x0,0x38(%rsp)
mov    0xc8c9a(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
mov    %r8,0x40(%rsp)
mov    $0x1,%cl
mov    %ecx,0x4c(%rsp)
cmp    %r14,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x53c>
mov    0x38(%rsp),%rbx
lea    (%r15,%rbx,8),%rsi
lea    (%r15,%rbx,8),%rdi
add    $0x8,%rdi
sub    %rbx,%r14
shl    $0x3,%r14
mov    %r14,%rdx
mov    %rax,%r12
call   *%rax
lea    0x1(%rbx),%rcx
mov    %rbp,(%r15,%rbx,8)
mov    $0x1,%ebp
mov    %r14,%rdx
mov    $0x1,%r14d
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc6f>
mov    0x408(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12d0>
movzwl 0x418(%rsp),%eax
mov    %rax,%rdx
cmp    $0x3,%rax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbf0>
cmp    $0x11,%ax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbd4>
lea    0x3(%rax),%edx
and    $0x3c,%edx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbf0>
mov    %rbx,0x30(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd75>
lea    0x1(%r14),%rcx
mov    %rcx,0x30(%rsp)
cmp    $0x10,%r14
mov    %r8,0x40(%rsp)
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x8da>
mov    $0x3,%eax
cmp    $0x3,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x591>
lea    0x4(%r14),%eax
and    $0x1c,%eax
lea    0x3(%r14),%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb3f>
mov    0x30(%rsp),%rcx
cmp    $0x11,%rcx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd5c>
mov    %rcx,%rax
lea    0x3(%rax),%ebx
and    $0x3c,%ebx
movl   $0x0,0x4c(%rsp)
mov    %rax,0x30(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd75>
mov    $0xffffffff,%edx
shl    %cl,%edx
not    %edx
and    %eax,%edx
mov    %edx,%ecx
shr    $1,%ecx
and    $0x15555555,%ecx
sub    %ecx,%edx
mov    %edx,%ecx
and    $0x33333333,%ecx
shr    $0x2,%edx
and    $0x33333333,%edx
add    %ecx,%edx
mov    %edx,%ecx
shr    $0x4,%ecx
add    %edx,%ecx
and    $0xf0f0f0f,%ecx
imul   $0x1010101,%ecx,%ecx
shr    $0x18,%ecx
mov    %rcx,0x38(%rsp)
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x911>
mov    %eax,%ecx
shr    $1,%ecx
and    $0x55555555,%ecx
mov    %eax,%edx
sub    %ecx,%edx
mov    %edx,%ecx
and    $0x33333333,%ecx
shr    $0x2,%edx
and    $0x33333333,%edx
add    %ecx,%edx
mov    %edx,%ecx
shr    $0x4,%ecx
add    %edx,%ecx
and    $0xf0f0f0f,%ecx
imul   $0x1010101,%ecx,%edx
shr    $0x18,%edx
lea    -0x1(%rax),%ecx
mov    $0x1,%sil
test   %ecx,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xca0>
cmp    $0x10,%edx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd16>
mov    $0x20,%r12d
cmp    $0x2,%edx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xca6>
lea    0x4(%rdx),%eax
lea    0x3(%rdx),%ecx
xor    %eax,%ecx
mov    0xc8aee(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
cmp    $0x4,%ecx
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1983>
lea    0x20(,%rdx,8),%r12d
and    $0xe0,%r12d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xca6>
mov    %r14,%rcx
xor    %eax,%eax
cs nopw 0x0(%rax,%rax,1)
mov    %rax,%rdx
mov    %rcx,%rsi
shr    $1,%rsi
add    %rsi,%rax
cmp    %rbp,(%r15,%rax,8)
cmova  %rdx,%rax
sub    %rsi,%rcx
cmp    $0x1,%rcx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x690>
cmp    %rbp,(%r15,%rax,8)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe8>
adc    $0x0,%rax
mov    %rax,0x38(%rsp)
cmp    $0x1f,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x194>
mov    0x180(%rsp),%eax
test   %eax,%eax
mov    %r8,0x40(%rsp)
mov    %rbp,0x50(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x7ba>
xorps  %xmm0,%xmm0
movaps %xmm0,0x70(%rsp)
lea    0x0(,%r14,8),%rbx
xor    %ebp,%ebp
data16 cs nopw 0x0(%rax,%rax,1)
mov    (%r15,%rbp,1),%rcx
mov    (%r12,%rbp,1),%r8
xorps  %xmm0,%xmm0
movaps %xmm0,0xf0(%rsp)
movaps %xmm0,0xe0(%rsp)
movaps %xmm0,0xd0(%rsp)
movaps %xmm0,0xc0(%rsp)
movq   $0xffffffffffffffff,0x100(%rsp)
lea    0x108(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0xc0(%rsp),%rax
mov    %rax,(%rsp)
lea    0x140(%rsp),%rdi
lea    0x178(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
add    $0x8,%rbp
cmp    %rbp,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x700>
lea    0x3a8(%rsp),%rax
mov    %rax,(%rsp)
lea    0xc0(%rsp),%rdi
lea    0x178(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    0x50(%rsp),%rcx
mov    0x40(%rsp),%r8
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa25>
movzbl 0x3a0(%rsp),%eax
xorps  %xmm0,%xmm0
test   %al,%al
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x952>
movaps %xmm0,0x70(%rsp)
movl   $0x0,0x8c(%rsp)
lea    0x0(,%r14,8),%rbx
xor    %ebp,%ebp
nopw   0x0(%rax,%rax,1)
mov    (%r15,%rbp,1),%rcx
mov    (%r12,%rbp,1),%r8
xorps  %xmm0,%xmm0
movaps %xmm0,0xf0(%rsp)
movaps %xmm0,0xe0(%rsp)
movaps %xmm0,0xd0(%rsp)
movaps %xmm0,0xc0(%rsp)
movq   $0xffffffffffffffff,0x100(%rsp)
lea    0x108(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0x8c(%rsp),%rax
mov    %rax,0x10(%rsp)
lea    0xc0(%rsp),%rax
mov    %rax,(%rsp)
movq   $0x1,0x8(%rsp)
lea    0x140(%rsp),%rdi
lea    0x178(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, false>>
add    $0x8,%rbp
cmp    %rbp,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x7f0>
lea    0x8c(%rsp),%rax
mov    %rax,0x10(%rsp)
lea    0x3a8(%rsp),%rax
mov    %rax,(%rsp)
movq   $0x1,0x8(%rsp)
lea    0xc0(%rsp),%rdi
lea    0x178(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    0x50(%rsp),%rcx
mov    0x40(%rsp),%r8
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa25>
cmp    $0x18,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb24>
cmp    $0x10,%r14
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb46>
mov    $0x18,%eax
mov    $0x11,%ecx
mov    %rcx,0x30(%rsp)
lea    0x3(%r14),%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x57f>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb3f>
addq   $0x10,0x378(%rsp)
incq   0x380(%rsp)
incq   0x390(%rsp)
mov    0x1b8(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x119c>
mov    (%rdi),%rax
mov    %rax,0x1b8(%rsp)
movq   $0x0,(%rdi)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x190d>
movaps %xmm0,0x70(%rsp)
lea    0x0(,%r14,8),%rbx
xor    %ebp,%ebp
data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    (%r15,%rbp,1),%rcx
mov    (%r12,%rbp,1),%r8
xorps  %xmm0,%xmm0
movaps %xmm0,0xf0(%rsp)
movaps %xmm0,0xe0(%rsp)
movaps %xmm0,0xd0(%rsp)
movaps %xmm0,0xc0(%rsp)
movq   $0xffffffffffffffff,0x100(%rsp)
lea    0x108(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0xc0(%rsp),%rax
mov    %rax,(%rsp)
lea    0x140(%rsp),%rdi
lea    0x178(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, true>>
add    $0x8,%rbp
cmp    %rbp,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x970>
lea    0x3a8(%rsp),%rax
mov    %rax,(%rsp)
lea    0xc0(%rsp),%rdi
lea    0x178(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    0x50(%rsp),%rcx
mov    0x40(%rsp),%r8
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, true>>
movaps 0x70(%rsp),%xmm0
movaps %xmm0,0x140(%rsp)
mov    %r14,%rbx
shl    $0x4,%rbx
add    $0x30,%rbx
and    $0xffffffffffffffc0,%rbx
cmp    $0x21,%r14
mov    $0x200,%eax
cmovb  %rax,%rbx
mov    0x180(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xab0>
cmp    $0x177,%rbx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xafe>
lea    -0x2a86d4(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rbx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb8a>
sub    %rbx,0x378(%rsp)
decq   0x380(%rsp)
cmp    $0x3e,%dil
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1afc>
mov    0x188(%rsp,%rdi,8),%rax
mov    %rax,(%r15)
mov    %r15,0x188(%rsp,%rdi,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xba5>
mov    0x178(%rsp),%r12
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a23>
shl    $0x6,%rax
lock sub %rbx,(%r12,%rax,1)
lock decq 0x8(%r12,%rax,1)
mov    0x1000(%r12),%rdi
add    $0x40,%rdi
mov    $0x10,%ecx
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xc8897(%rip)        # ADDR <_DYNAMIC+0x528>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xba5>
lock sub %rbx,0x378(%rsp)
lock decq 0x380(%rsp)
movabs $0x7ffffffffffffff1,%rax
cmp    %rax,%rbx
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb9c>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1aba>
setne  %al
movzbl %al,%eax
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
cmp    %eax,%ecx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x57f>
mov    0xc860a(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
mov    0x38(%rsp),%rbx
lea    (%r15,%rbx,8),%rsi
lea    0x1(%rbx),%rcx
mov    %rcx,0x20(%rsp)
lea    (%r15,%rbx,8),%rdi
add    $0x8,%rdi
mov    %r14,%rdx
sub    %rbx,%rdx
shl    $0x3,%rdx
mov    %rdx,%r12
call   *%rax
mov    %rbp,(%r15,%rbx,8)
cmp    $0x11,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc45>
add    $0x3,%r14d
and    $0x3c,%r14d
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc5b>
lock sub %rbx,0x378(%rsp)
lock decq 0x380(%rsp)
mov    %r15,%rdi
call   *0xc84f3(%rip)        # ADDR <free@GLIBC_2.2.5>
inc    %r14
mov    %r14,0x170(%rsp)
movaps 0x140(%rsp),%xmm0
lea    0x160(%rsp),%rax
movups %xmm0,(%rax)
movq   $0x2,0x158(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
mov    $0x18,%edx
cmp    $0x19,%ax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbf0>
mov    $0x20,%edx
cmp    $0x21,%ax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbf0>
lea    0x3(%rax),%edx
and    $0xfffffffc,%edx
lea    (%rcx,%rdx,8),%rdx
cmp    %bpl,-0x1(%rax,%rdx,1)
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc35>
lea    -0x19(%rax),%esi
cmp    $0xffe9,%si
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12d0>
cmp    $0x10,%ax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12b5>
cmp    $0x3,%ax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12d0>
lea    0x4(%rax),%esi
and    $0x1c,%esi
lea    0x3(%rax),%edi
and    $0x3c,%edi
cmp    %edi,%esi
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12d0>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19c7>
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12d0>
mov    %r8,-0x8(%rcx,%rax,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
lea    0x18(,%rax,8),%r14
mov    0x38(%rsp),%rbx
mov    %r12,%rdx
mov    0xc84eb(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    0x30(%rsp),%rbp
mov    0x20(%rsp),%rcx
lea    (%r15,%r14,8),%r14
lea    (%r14,%rbx,8),%rsi
lea    (%r14,%rcx,8),%rdi
call   *%r12
mov    0x40(%rsp),%rax
mov    %rax,(%r14,%rbx,8)
movq   $0x1,0x158(%rsp)
mov    %rbp,0x168(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
mov    $0x10,%r12d
lea    -0x2a8914(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%r12,%rax,1),%r15d
cmp    $0xff,%r15
mov    %r8,0x40(%rsp)
mov    %rbp,0x50(%rsp)
mov    %rbx,0x20(%rsp)
mov    %rdx,0x30(%rsp)
mov    %esi,0x4c(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd3c>
add    %r12,0x378(%rsp)
incq   0x380(%rsp)
incq   0x390(%rsp)
cmp    $0x3e,%r15b
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b03>
mov    0x188(%rsp,%r15,8),%rdi
test   %rdi,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x161c>
mov    (%rdi),%rax
mov    %rax,0x188(%rsp,%r15,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17b4>
cmp    $0x18,%edx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1961>
cmp    $0x10,%edx
mov    0xc8427(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1983>
mov    $0xc0,%r12d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xca6>
lock add %r12,0x378(%rsp)
lock incq 0x380(%rsp)
lock incq 0x390(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17c7>
xor    %eax,%eax
cmp    $0x19,%rcx
setae  %al
lea    0x18(,%rax,8),%rbx
movl   $0x0,0x4c(%rsp)
mov    %ebx,%eax
shl    $0x4,%eax
mov    %rax,0x28(%rsp)
mov    0x180(%rsp),%eax
test   %eax,%eax
mov    %rbp,0x50(%rsp)
mov    %r13,0x98(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe47>
cmp    $0x17,%ebx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe06>
lea    -0x2a8a0e(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
mov    0x28(%rsp),%rcx
movzbl (%rcx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe06>
mov    0x28(%rsp),%rax
add    %rax,0x378(%rsp)
incq   0x380(%rsp)
incq   0x390(%rsp)
cmp    $0x3e,%dil
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b06>
mov    0x188(%rsp,%rdi,8),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xed8>
mov    (%rax),%rcx
mov    %rcx,0x188(%rsp,%rdi,8)
mov    0x28(%rsp),%rbp
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xfb1>
mov    0x28(%rsp),%rbx
lock add %rbx,0x378(%rsp)
lock incq 0x380(%rsp)
lock incq 0x390(%rsp)
mov    $0x1,%esi
mov    %rbx,%rdi
call   *0xc8334(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,0x20(%rsp)
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xfc4>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b47>
mov    0x178(%rsp),%rcx
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a2e>
shl    $0x6,%rax
mov    0x28(%rsp),%rbp
lock add %rbp,(%rcx,%rax,1)
lock incq 0x8(%rcx,%rax,1)
lock incq 0x10(%rcx,%rax,1)
cmp    $0x17,%ebx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xeb7>
lea    -0x2a8af1(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl 0x0(%rbp,%rax,1),%esi
cmp    $0xff,%rsi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xeb7>
mov    0x1000(%rcx),%rdi
add    $0x40,%rdi
call   *0xc84df(%rip)        # ADDR <_DYNAMIC+0x520>
test   %rax,%rax
mov    0x28(%rsp),%rbp
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xfb1>
mov    $0x1,%esi
mov    %rbp,%rdi
call   *0xc82a3(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,0x20(%rsp)
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xfc4>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4a>
cmp    $0x10,%ebx
mov    0x28(%rsp),%rbx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe26>
mov    %rdi,0x20(%rsp)
movq   $0x0,0xc0(%rsp)
mov    $0x40,%ebx
mov    $0x1000,%ebp
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xc0(%rsp),%rdi
call   *0xc8247(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    0xc0(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    $0x1000,%edx
mov    %rax,%rbx
mov    %rax,%rdi
xor    %esi,%esi
call   *0xc822b(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x398(%rsp),%rax
mov    %rax,(%rbx)
mov    0x20(%rsp),%rdi
mov    %rdi,0x8(%rbx)
mov    %rbx,0x398(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
mov    0x28(%rsp),%rbp
div    %bp
movzwl %ax,%r8d
mov    %rbx,%rax
cs nopw 0x0(%rax,%rax,1)
dec    %r8
mov    %r8,%rcx
imul   %rbp,%rcx
lea    (%rax,%rcx,1),%rdx
add    $0x40,%rdx
mov    0x188(%rsp,%rdi,8),%rsi
mov    %rsi,0x40(%rax,%rcx,1)
mov    %rdx,0x188(%rsp,%rdi,8)
cmp    $0x1,%r8
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf80>
add    $0x40,%rax
mov    %rax,0x20(%rsp)
mov    %rax,%rdi
xor    %esi,%esi
mov    %rbp,%rdx
call   *0xc81ac(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x38(%rsp),%rbx
lea    0x0(,%rbx,8),%rdx
mov    0x20(%rsp),%r13
mov    %r13,%rdi
mov    %r15,%rsi
mov    %rdx,0x28(%rsp)
mov    0xc8098(%rip),%rbp        # ADDR <memcpy@GLIBC_2.14>
call   *%rbp
mov    %rbp,%rax
mov    0x50(%rsp),%rcx
mov    %rcx,0x0(%r13,%rbx,8)
lea    0x8(,%rbx,8),%rdi
add    %r13,%rdi
lea    (%r15,%rbx,8),%rsi
mov    %r14,%rbp
sub    %rbx,%rbp
shl    $0x3,%rbp
mov    %rbp,%rdx
call   *%rax
mov    0x30(%rsp),%rax
cmpb   $0x0,0x4c(%rsp)
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1045>
mov    0x30(%rsp),%rcx
cmp    $0x11,%rcx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1034>
lea    0x3(%rcx),%eax
and    $0x3c,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1045>
xor    %eax,%eax
cmp    $0x19,%rcx
setae  %al
lea    0x18(,%rax,8),%rax
mov    0x38(%rsp),%r13
mov    0x20(%rsp),%rcx
lea    (%rcx,%rax,8),%rbx
mov    %rbx,%rdi
mov    %r12,%rsi
mov    0x28(%rsp),%rdx
call   *0xc801c(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x40(%rsp),%rax
mov    %rax,(%rbx,%r13,8)
lea    0x1(%r13),%rax
lea    (%rbx,%rax,8),%rdi
lea    (%r12,%r13,8),%rsi
mov    %rbp,%rdx
call   *0xc7ffe(%rip)        # ADDR <memcpy@GLIBC_2.14>
cmp    $0x3,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x10a9>
cmp    $0x11,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1098>
add    $0x3,%r14d
and    $0x3c,%r14d
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x10a9>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
lea    0x18(,%rax,8),%r14
mov    0x98(%rsp),%r13
mov    0x20(%rsp),%r12
mov    %r14,%rbx
shl    $0x4,%rbx
mov    0x180(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x112e>
cmp    $0x17,%r14
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1111>
lea    -0x2a8d3c(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rbx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1111>
sub    %rbx,0x378(%rsp)
decq   0x380(%rsp)
cmp    $0x3e,%dil
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1afc>
mov    0x188(%rsp,%rdi,8),%rax
mov    %rax,(%r15)
mov    %r15,0x188(%rsp,%rdi,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1176>
lock sub %rbx,0x378(%rsp)
lock decq 0x380(%rsp)
mov    %r15,%rdi
call   *0xc7f6c(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1176>
mov    0x178(%rsp),%r14
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a3f>
shl    $0x6,%rax
lock sub %rbx,(%r14,%rax,1)
lock decq 0x8(%r14,%rax,1)
mov    0x1000(%r14),%rdi
add    $0x40,%rdi
mov    $0x10,%ecx
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xc821a(%rip)        # ADDR <_DYNAMIC+0x528>
movq   $0x1,0x158(%rsp)
mov    %r12,0x160(%rsp)
mov    0x30(%rsp),%rax
mov    %rax,0x168(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
mov    %rbx,%r12
mov    %rbp,0x50(%rsp)
mov    %r8,0x40(%rsp)
movq   $0x0,0xc0(%rsp)
mov    $0x40,%ebx
mov    $0x1000,%ebp
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xc0(%rsp),%rdi
call   *0xc7f89(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    0xc0(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    $0x1000,%edx
xor    %esi,%esi
mov    %rdi,%rbx
call   *0xc7f70(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %rbx,%rdi
mov    0x398(%rsp),%rax
mov    %rax,(%rbx)
movq   $0x6,0x8(%rbx)
mov    %rbx,0x398(%rsp)
lea    0xff0(%rbx),%rax
mov    $0xffffffffffffff08,%rcx
mov    %r12,%rbx
nop
mov    0x1b8(%rsp),%rdx
mov    %rdx,(%rax)
mov    %rax,0x1b8(%rsp)
lea    -0x10(%rax),%rdx
mov    0x1b8(%rsp),%rsi
mov    %rsi,-0x10(%rax)
mov    %rdx,0x1b8(%rsp)
lea    -0x20(%rax),%rdx
mov    0x1b8(%rsp),%rsi
mov    %rsi,-0x20(%rax)
mov    %rdx,0x1b8(%rsp)
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x129a>
lea    -0x30(%rax),%rdx
mov    0x1b8(%rsp),%rsi
mov    %rsi,-0x30(%rax)
mov    %rdx,0x1b8(%rsp)
add    $0xffffffffffffffc0,%rax
add    $0x4,%rcx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1230>
movq   $0x0,0x40(%rdi)
add    $0x40,%rdi
mov    0x40(%rsp),%r8
mov    0x50(%rsp),%rbp
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x190d>
cmp    $0x10,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19b4>
cmp    $0x18,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19c7>
nopl   0x0(%rax,%rax,1)
mov    0x3f8(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x153e>
mov    0x410(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1511>
movq   $0x0,0x410(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1511>
movzbl 0x3f1(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1349>
mov    0x3b0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2ab0b4(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r9,%r8
cmp    $0x2,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1511>
movzbl 0x3f2(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1394>
mov    0x3b8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2ab0ff(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r9,%r8
cmp    $0x3,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1511>
movzbl 0x3f3(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x13df>
mov    0x3c0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2ab14a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r9,%r8
cmp    $0x4,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1511>
movzbl 0x3f4(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x142a>
mov    0x3c8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2ab195(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r9,%r8
cmp    $0x5,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1511>
movzbl 0x3f5(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1475>
mov    0x3d0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2ab1e0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r9,%r8
cmp    $0x6,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1511>
movzbl 0x3f6(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x14c0>
mov    0x3d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2ab22b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r9,%r8
cmp    $0x7,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1511>
movzbl 0x3f7(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1507>
mov    0x3e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2ab272(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r9,%r8
cmp    $0x8,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b2a>
movq   $0xffffffffffffffff,0x3e8(%rsp)
movw   $0x0,0x418(%rsp)
xorps  %xmm0,%xmm0
lea    0x3a8(%rsp),%rax
movups %xmm0,0x50(%rax)
movq   $0x0,0x60(%rax)
mov    0x180(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1580>
lea    0x3a8(%rsp),%rax
mov    %rax,(%rsp)
lea    0xc0(%rsp),%rdi
lea    0x178(%rsp),%rsi
lea    0x160(%rsp),%rdx
mov    %rbp,%rcx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1600>
movzbl 0x3a0(%rsp),%eax
test   %al,%al
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x15c9>
lea    0x3a8(%rsp),%rax
mov    %rax,(%rsp)
movq   $0x0,0x8(%rsp)
lea    0xc0(%rsp),%rdi
lea    0x178(%rsp),%rsi
lea    0x160(%rsp),%rdx
mov    %rbp,%rcx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1600>
lea    0x3a8(%rsp),%rax
mov    %rax,(%rsp)
lea    0xc0(%rsp),%rdi
lea    0x178(%rsp),%rsi
lea    0x160(%rsp),%rdx
mov    %rbp,%rcx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, true>>
nopl   0x0(%rax,%rax,1)
cmpq   $0x0,0xc0(%rsp)
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
incq   0x170(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
cmp    $0x100,%r12d
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17c7>
movq   $0x0,0xc0(%rsp)
mov    $0x40,%ebx
mov    $0x1000,%ebp
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xc0(%rsp),%rdi
call   *0xc7b09(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    0xc0(%rsp),%rdi
test   %rdi,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    $0x1000,%edx
xor    %esi,%esi
mov    %rdi,%rbp
call   *0xc7af0(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x398(%rsp),%rax
mov    %rax,0x0(%rbp)
mov    %r15,0x8(%rbp)
mov    %rbp,0x398(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x16ec>
mov    %rbp,%r10
mov    %r15,%r9
mov    %rcx,%rax
nopw   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rsi
imul   %r12,%rsi
lea    (%r10,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x188(%rsp,%r9,8),%r8
mov    %r8,0x40(%r10,%rsi,1)
mov    %rdi,0x188(%rsp,%r9,8)
dec    %rdx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x16c0>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
mov    0x20(%rsp),%rbx
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17ad>
lea    -0x4(%rax),%rcx
imul   %r12,%rcx
add    $0x40,%rcx
lea    0x0(,%r12,4),%edx
neg    %rdx
lea    -0x3(%rax),%rsi
imul   %r12,%rsi
add    $0x40,%rsi
lea    -0x2(%rax),%rdi
imul   %r12,%rdi
add    $0x40,%rdi
lea    -0x1(%rax),%r8
imul   %r12,%r8
add    $0x40,%r8
mov    %rbp,%r9
nopl   0x0(%rax)
lea    (%r9,%r8,1),%r10
mov    0x188(%rsp,%r15,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x188(%rsp,%r15,8)
lea    (%r9,%rdi,1),%r10
mov    0x188(%rsp,%r15,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x188(%rsp,%r15,8)
lea    (%r9,%rsi,1),%r10
mov    0x188(%rsp,%r15,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x188(%rsp,%r15,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x188(%rsp,%r15,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x188(%rsp,%r15,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1740>
mov    %rbp,%rdi
add    $0x40,%rdi
xor    %esi,%esi
mov    %r12,%rdx
mov    %rdi,%r15
call   *0xc79ae(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r15,%rdi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17e1>
mov    $0x1,%esi
mov    %r12,%rdi
call   *0xc7993(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%rdi
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b3d>
mov    %r14,0x28(%rsp)
mov    0x20(%rbx,%r14,8),%rbx
mov    0x38(%rsp),%r15
lea    0x0(,%r15,8),%r12d
mov    %rbx,%rsi
mov    %r12,%rdx
mov    0xc787b(%rip),%rbp        # ADDR <memcpy@GLIBC_2.14>
mov    %rdi,%r14
call   *%rbp
mov    %r14,0xa0(%rsp)
lea    (%r14,%r12,1),%rdi
add    $0x8,%rdi
add    %rbx,%r12
mov    0x30(%rsp),%rdx
sub    %r15,%rdx
shl    $0x3,%rdx
mov    %r12,%rsi
call   *%rbp
test   %rbx,%rbx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1ae1>
cmpb   $0x0,0x4c(%rsp)
mov    0x50(%rsp),%rbp
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x184f>
mov    0x30(%rsp),%rdx
mov    0x28(%rsp),%r14
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1864>
mov    0x30(%rsp),%rdx
cmp    $0x11,%edx
mov    0x28(%rsp),%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18b8>
add    $0x3,%edx
and    $0x3c,%edx
lea    0x0(,%rdx,8),%ecx
lea    0x8(,%rdx,8),%eax
and    $0xfffffff0,%eax
lea    -0x2a94e3(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rdx,1),%edi
cmp    $0xff,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18e0>
sub    %rax,0x378(%rsp)
decq   0x380(%rsp)
cmp    $0x3e,%dil
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1afc>
mov    0x188(%rsp,%rdi,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0x188(%rsp,%rdi,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18fb>
xor    %eax,%eax
cmp    $0x19,%edx
setae  %al
shl    $0x6,%eax
add    $0xc0,%rax
mov    %rax,%rcx
lea    -0x2a953a(%rip),%rdx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rdx,1),%edi
cmp    $0xff,%rdi
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1889>
lock sub %rax,0x378(%rsp)
lock decq 0x380(%rsp)
mov    %rbx,%rdi
call   *0xc779d(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x40(%rsp),%r8
mov    0x20(%rsp),%rbx
mov    0xa0(%rsp),%rdi
mov    0x38(%rsp),%rax
mov    %r8,(%rdi,%rax,8)
mov    %rdi,0x20(%rbx,%r14,8)
mov    $0x1,%eax
mov    %ebp,%ecx
shl    %cl,%rax
shr    $0x3,%ebp
and    $0x18,%ebp
or     %rax,(%rbx,%rbp,1)
incq   0x410(%rsp)
mov    0x418(%rsp),%eax
lea    0x1(%rax),%ecx
mov    %cx,0x418(%rsp)
incq   0x170(%rsp)
mov    0x3a8(%rsp),%rcx
mov    %al,0x8(%rcx)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
cmp    $0xffffffff,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a0b>
cmp    $0x18,%edx
mov    0xc77dc(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1983>
mov    $0x100,%r12d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xca6>
mov    %r8,%r15
mov    %rbx,%r12
mov    0x20(%rbx,%r14,8),%rbx
mov    0x38(%rsp),%r14
lea    (%rbx,%r14,8),%rsi
lea    (%rbx,%r14,8),%rdi
add    $0x8,%rdi
sub    %r14,%rdx
shl    $0x3,%rdx
call   *%rax
mov    %r15,(%rbx,%r14,8)
mov    %r12,%rbx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x191b>
mov    $0x18,%esi
lea    0x3(%rax),%edi
and    $0x3c,%edi
cmp    %edi,%esi
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12d0>
mov    %bpl,(%rdx,%rax,1)
mov    %r8,(%rcx,%rax,8)
mov    0x3a8(%rsp),%rcx
mov    0x8(%rcx),%rdx
mov    $0xffffffffffffff00,%rsi
and    %rsi,%rdx
or     %rax,%rdx
mov    %rdx,0x8(%rcx)
inc    %eax
mov    %ax,0x418(%rsp)
incq   0x410(%rsp)
incq   0x170(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
mov    $0x120,%r12d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xca6>
call   *0xc797a(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x2c3>
call   *0xc796f(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xacd>
mov    %rcx,%rbp
call   *0xc7961(%rip)        # ADDR <_DYNAMIC+0x530>
mov    %rbp,%rcx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe64>
call   *0xc7953(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x114b>
mov    0x158(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a70>
cmp    $0x1,%ebx
mov    0x60(%rsp),%r14
mov    0x58(%rsp),%rdi
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a7c>
mov    0x168(%rsp),%rbx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    0x60(%rsp),%r14
mov    0x58(%rsp),%rdi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    0x170(%rsp),%rbx
mov    %rbx,0xa8(%rsp)
lea    0xa8(%rsp),%rax
cmpq   $0x0,(%r14)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1aa0>
call   *0xc75f8(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0xa8(%rsp),%rax
add    $0x428,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
lea    -0x2ab8dc(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0xc4978(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0xbdb01(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x6f(%rsp),%rdx
mov    $0x11,%esi
call   *0xc7651(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b5b>
lea    -0x2ab830(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8d5>
lea    0xc4a09(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7158>
mov    $0x6,%esi
call   *0xc773e(%rip)        # ADDR <_DYNAMIC+0x3d0>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b5b>
mov    $0x3e,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b1a>
mov    %r15,%rdi
mov    $0x3e,%esi
lea    0xbdad6(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    %rax,0x90(%rsp)
mov    0x90(%rsp),%rdx
call   *0xc7548(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b5b>
mov    $0x8,%edi
mov    $0x8,%esi
lea    0xc4bb5(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b12>
mov    $0x10,%ebx
mov    %r12,%rbp
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4f>
mov    %rbx,%rbp
mov    $0x10,%ebx
mov    %rbx,%rdi
mov    %rbp,%rsi
call   *0xc758d(%rip)        # ADDR <_DYNAMIC+0x280>
ud2
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b65>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b65>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b65>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b65>
mov    %rax,%rbx
mov    0x60(%rsp),%r14
mov    0x58(%rsp),%r15
lea    0x158(%rsp),%rdi
call   ADDR <core::ptr::drop_glue::<expanse_trie::map::ExpanseMap>>
cmpq   $0x0,(%r14)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b8e>
mov    %r15,%rdi
call   *0xc750a(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
call   *0xc7564(%rip)        # ADDR <_DYNAMIC+0x298>
int3
int3
int3
int3
