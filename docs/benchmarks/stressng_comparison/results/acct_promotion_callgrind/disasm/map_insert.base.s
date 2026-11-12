push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x428,%rsp
mov    %rdi,%r14
lea    0x190(%rsp),%rdi
xor    %ebx,%ebx
mov    $0x1f0,%edx
xor    %esi,%esi
call   *0xc7845(%rip)        # ADDR <memset@GLIBC_2.2.5>
xorps  %xmm0,%xmm0
movups %xmm0,0x3b0(%rsp)
movups %xmm0,0x3c0(%rsp)
movups %xmm0,0x3d0(%rsp)
movups %xmm0,0x3e0(%rsp)
movq   $0x0,0x160(%rsp)
movq   $0x0,0x178(%rsp)
movl   $0x3,0x188(%rsp)
movups %xmm0,0x380(%rsp)
movups %xmm0,0x390(%rsp)
movups %xmm0,0x399(%rsp)
movq   $0xffffffffffffffff,0x3f0(%rsp)
movups %xmm0,0x3f8(%rsp)
movups %xmm0,0x408(%rsp)
movups %xmm0,0x412(%rsp)
mov    0x8(%r14),%rdi
mov    0x10(%r14),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19af>
mov    %r14,0x58(%rsp)
lea    (%rdi,%rax,8),%rax
mov    %rax,0x158(%rsp)
lea    0xbdbe2(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    %rax,0x88(%rsp)
mov    %rdi,0x50(%rsp)
mov    %rdi,%r13
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x102>
mov    %r8,(%r12,%rax,8)
nopl   0x0(%rax)
add    $0x8,%r13
cmp    0x158(%rsp),%r13
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1975>
mov    0x0(%r13),%rax
mov    %rax,0xa0(%rsp)
lea    0xa0(%rsp),%rcx
not    %rax
mov    %rax,0xa8(%rsp)
lea    0xa8(%rsp),%rax
mov    0xa0(%rsp),%rbp
mov    0xa8(%rsp),%r8
mov    0x160(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b0>
cmp    $0x1,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x200>
mov    0x168(%rsp),%r15
mov    0x170(%rsp),%r14
cmp    $0x3,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x290>
lea    (%r15,%r14,8),%r12
test   %r14,%r14
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x4a1>
lea    -0x1(%r14),%rax
mov    %r14,0x38(%rsp)
cmp    -0x8(%r15,%r14,8),%rbp
jbe    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x361>
cmp    $0x1f,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x6ba>
cmp    $0x1,%r14
mov    0xc76b1(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x549>
mov    $0x2,%ebx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x4b6>
nop
mov    0x188(%rsp),%eax
mov    %r8,%r15
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x2a6>
lock addq $0x10,0x380(%rsp)
lock incq 0x388(%rsp)
lock incq 0x398(%rsp)
mov    0x200(%rsp),%r14
test   %r14,%r14
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x378>
mov    (%r14),%rax
mov    %rax,0x200(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x46e>
nop
mov    %rbp,%rax
shr    $0x8,%rax
cmp    %rax,0x3f0(%rsp)
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1240>
mov    0x408(%rsp),%rbx
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
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x5ac>
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
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x32b>
lea    0x3(%r14),%eax
and    $0x3c,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x349>
mov    0x180(%rsp),%rbx
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1949>
shl    $0x6,%rax
lock addq $0x10,(%rbx,%rax,1)
lock incq 0x8(%rbx,%rax,1)
lock incq 0x10(%rbx,%rax,1)
mov    0x1000(%rbx),%rdi
add    $0x40,%rdi
mov    $0xe,%esi
call   *0xc7799(%rip)        # ADDR <_DYNAMIC+0x520>
mov    %rax,%r14
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x46e>
mov    $0x10,%eax
mov    %rax,0x20(%rsp)
mov    $0x10,%edi
mov    $0x1,%esi
call   *0xc7553(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r14
mov    $0x10,%ebx
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x475>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    $0x18,%eax
cmp    $0x19,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x349>
mov    $0x20,%eax
cmp    $0x21,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x349>
lea    0x3(%r14),%rax
and    $0xfffffffffffffffc,%rax
lea    (%r15,%rax,8),%r12
lea    -0x1(%r14),%rax
mov    %r14,0x38(%rsp)
cmp    -0x8(%r15,%r14,8),%rbp
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18a>
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe8>
cmp    $0x1,%r14
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x67b>
xor    %eax,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x69d>
movq   $0x0,0xb0(%rsp)
mov    $0x40,%eax
mov    %rax,0x20(%rsp)
mov    $0x1000,%ebx
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xb0(%rsp),%rdi
call   *0xc74b5(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    0xb0(%rsp),%r14
test   %r14,%r14
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    $0x1000,%edx
mov    %r14,%rdi
xor    %esi,%esi
call   *0xc749c(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x3a0(%rsp),%rax
mov    %rax,(%r14)
movq   $0xe,0x8(%r14)
mov    %r14,0x3a0(%rsp)
lea    0xff0(%r14),%rax
mov    $0xffffffffffffff08,%rcx
nopl   (%rax)
mov    0x200(%rsp),%rdx
mov    %rdx,(%rax)
mov    %rax,0x200(%rsp)
lea    -0x10(%rax),%rdx
mov    0x200(%rsp),%rsi
mov    %rsi,-0x10(%rax)
mov    %rdx,0x200(%rsp)
lea    -0x20(%rax),%rdx
mov    0x200(%rsp),%rsi
mov    %rsi,-0x20(%rax)
mov    %rdx,0x200(%rsp)
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x46a>
lea    -0x30(%rax),%rdx
mov    0x200(%rsp),%rsi
mov    %rsi,-0x30(%rax)
mov    %rdx,0x200(%rsp)
add    $0xffffffffffffffc0,%rax
add    $0x4,%rcx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x400>
add    $0x40,%r14
xorps  %xmm0,%xmm0
movups %xmm0,(%r14)
mov    %rbp,(%r14)
mov    %r15,0x8(%r14)
movq   $0x1,0x160(%rsp)
mov    %r14,0x168(%rsp)
movq   $0x1,0x170(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
mov    $0x1,%ebx
movq   $0x0,0x38(%rsp)
mov    0xc739a(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
mov    %r8,0x40(%rsp)
mov    $0x1,%cl
mov    %ecx,0x60(%rsp)
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
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc2a>
mov    0x410(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1240>
movzwl 0x420(%rsp),%eax
mov    %rax,%rdx
cmp    $0x3,%rax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbab>
cmp    $0x11,%ax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb8f>
lea    0x3(%rax),%edx
and    $0x3c,%edx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbab>
mov    %rbx,0x30(%rsp)
mov    %rbx,%rcx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd18>
lea    0x1(%r14),%rcx
mov    %rcx,0x30(%rsp)
cmp    $0x10,%r14
mov    %r8,0x40(%rsp)
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x8ca>
mov    $0x3,%eax
cmp    $0x3,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x594>
lea    0x4(%r14),%eax
and    $0x1c,%eax
lea    0x3(%r14),%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb48>
mov    0x30(%rsp),%rcx
cmp    $0x11,%rcx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xcff>
mov    %rcx,%rax
lea    0x3(%rax),%ecx
and    $0x3c,%ecx
movl   $0x0,0x60(%rsp)
mov    %rax,0x30(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd18>
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
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x901>
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
test   %ecx,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc5b>
cmp    $0x10,%edx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xcdb>
cmp    $0x2,%rdx
sete   %sil
cmp    $0x2,%edx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x662>
lea    0x4(%rdx),%eax
lea    0x3(%rdx),%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18ab>
mov    %rbp,0x28(%rsp)
lea    0x20(,%rdx,8),%r12d
and    $0xe0,%r12d
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc69>
mov    %r14,%rcx
xor    %eax,%eax
mov    %rax,%rdx
mov    %rcx,%rsi
shr    $1,%rsi
add    %rsi,%rax
cmp    %rbp,(%r15,%rax,8)
cmova  %rdx,%rax
sub    %rsi,%rcx
cmp    $0x1,%rcx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x680>
cmp    %rbp,(%r15,%rax,8)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe8>
adc    $0x0,%rax
mov    %rax,0x38(%rsp)
cmp    $0x1f,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x194>
mov    0x188(%rsp),%eax
test   %eax,%eax
mov    %r8,0x40(%rsp)
mov    %rbp,0x28(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x7aa>
xorps  %xmm0,%xmm0
movaps %xmm0,0x70(%rsp)
lea    0x0(,%r14,8),%rbx
xor    %ebp,%ebp
data16 cs nopw 0x0(%rax,%rax,1)
mov    (%r15,%rbp,1),%rcx
mov    (%r12,%rbp,1),%r8
xorps  %xmm0,%xmm0
movaps %xmm0,0xe0(%rsp)
movaps %xmm0,0xd0(%rsp)
movaps %xmm0,0xc0(%rsp)
movaps %xmm0,0xb0(%rsp)
movq   $0xffffffffffffffff,0xf0(%rsp)
lea    0xf8(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0xb0(%rsp),%rax
mov    %rax,(%rsp)
lea    0x130(%rsp),%rdi
lea    0x180(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
add    $0x8,%rbp
cmp    %rbp,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x6f0>
lea    0x3b0(%rsp),%rax
mov    %rax,(%rsp)
lea    0xb0(%rsp),%rdi
lea    0x180(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    0x28(%rsp),%rcx
mov    0x40(%rsp),%r8
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa15>
movzbl 0x3a8(%rsp),%eax
xorps  %xmm0,%xmm0
test   %al,%al
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x945>
movaps %xmm0,0x70(%rsp)
movl   $0x0,0x84(%rsp)
lea    0x0(,%r14,8),%rbx
xor    %ebp,%ebp
nopw   0x0(%rax,%rax,1)
mov    (%r15,%rbp,1),%rcx
mov    (%r12,%rbp,1),%r8
xorps  %xmm0,%xmm0
movaps %xmm0,0xe0(%rsp)
movaps %xmm0,0xd0(%rsp)
movaps %xmm0,0xc0(%rsp)
movaps %xmm0,0xb0(%rsp)
movq   $0xffffffffffffffff,0xf0(%rsp)
lea    0xf8(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0x84(%rsp),%rax
mov    %rax,0x10(%rsp)
lea    0xb0(%rsp),%rax
mov    %rax,(%rsp)
movq   $0x1,0x8(%rsp)
lea    0x130(%rsp),%rdi
lea    0x180(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, false>>
add    $0x8,%rbp
cmp    %rbp,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x7e0>
lea    0x84(%rsp),%rax
mov    %rax,0x10(%rsp)
lea    0x3b0(%rsp),%rax
mov    %rax,(%rsp)
movq   $0x1,0x8(%rsp)
lea    0xb0(%rsp),%rdi
lea    0x180(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    0x28(%rsp),%rcx
mov    0x40(%rsp),%r8
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa15>
cmp    $0x18,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb2d>
cmp    $0x10,%r14
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb4f>
mov    $0x18,%eax
mov    $0x11,%ecx
mov    %rcx,0x30(%rsp)
lea    0x3(%r14),%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x582>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb48>
lock addq $0x10,0x380(%rsp)
lock incq 0x388(%rsp)
lock incq 0x398(%rsp)
mov    0x1c0(%rsp),%r15
test   %r15,%r15
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1102>
mov    (%r15),%rax
mov    %rax,0x1c0(%rsp)
movq   $0x0,(%r15)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1837>
movaps %xmm0,0x70(%rsp)
lea    0x0(,%r14,8),%rbx
xor    %ebp,%ebp
data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    (%r15,%rbp,1),%rcx
mov    (%r12,%rbp,1),%r8
xorps  %xmm0,%xmm0
movaps %xmm0,0xe0(%rsp)
movaps %xmm0,0xd0(%rsp)
movaps %xmm0,0xc0(%rsp)
movaps %xmm0,0xb0(%rsp)
movq   $0xffffffffffffffff,0xf0(%rsp)
lea    0xf8(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0xb0(%rsp),%rax
mov    %rax,(%rsp)
lea    0x130(%rsp),%rdi
lea    0x180(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, true>>
add    $0x8,%rbp
cmp    %rbp,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x960>
lea    0x3b0(%rsp),%rax
mov    %rax,(%rsp)
lea    0xb0(%rsp),%rdi
lea    0x180(%rsp),%rsi
lea    0x70(%rsp),%rdx
mov    0x28(%rsp),%rcx
mov    0x40(%rsp),%r8
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, true>>
movaps 0x70(%rsp),%xmm0
movaps %xmm0,0x130(%rsp)
mov    %r14,%rbx
shl    $0x4,%rbx
add    $0x30,%rbx
and    $0xffffffffffffffc0,%rbx
cmp    $0x21,%r14
mov    $0x200,%eax
cmovb  %rax,%rbx
mov    0x188(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa97>
lock sub %rbx,0x380(%rsp)
lock decq 0x388(%rsp)
cmp    $0x177,%rbx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xae2>
lea    -0x2a21b2(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rbx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xaf5>
cmp    $0x3e,%dil
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a2a>
mov    0x190(%rsp,%rdi,8),%rax
mov    %rax,(%r15)
mov    %r15,0x190(%rsp,%rdi,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xafe>
mov    0x180(%rsp),%r12
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1954>
shl    $0x6,%rax
lock sub %rbx,(%r12,%rax,1)
lock decq 0x8(%r12,%rax,1)
mov    0x1000(%r12),%rdi
add    $0x40,%rdi
mov    $0x10,%ecx
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xc6fb0(%rip)        # ADDR <_DYNAMIC+0x528>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xafe>
movabs $0x7ffffffffffffff1,%rax
cmp    %rax,%rbx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19e5>
mov    %r15,%rdi
call   *0xc6c9a(%rip)        # ADDR <free@GLIBC_2.2.5>
inc    %r14
mov    %r14,0x178(%rsp)
movaps 0x130(%rsp),%xmm0
lea    0x168(%rsp),%rax
movups %xmm0,(%rax)
movq   $0x2,0x160(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
setne  %al
movzbl %al,%eax
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
cmp    %eax,%ecx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x582>
mov    0xc6d01(%rip),%rax        # ADDR <memmove@GLIBC_2.2.5>
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
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc00>
add    $0x3,%r14d
and    $0x3c,%r14d
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc16>
mov    $0x18,%edx
cmp    $0x19,%ax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbab>
mov    $0x20,%edx
cmp    $0x21,%ax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbab>
lea    0x3(%rax),%edx
and    $0xfffffffc,%edx
lea    (%rcx,%rdx,8),%rdx
cmp    %bpl,-0x1(%rax,%rdx,1)
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xbf0>
lea    -0x19(%rax),%esi
cmp    $0xffe9,%si
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1240>
cmp    $0x10,%ax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1223>
cmp    $0x3,%ax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1240>
lea    0x4(%rax),%esi
and    $0x1c,%esi
lea    0x3(%rax),%edi
and    $0x3c,%edi
cmp    %edi,%esi
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1240>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18f3>
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1240>
mov    %r8,-0x8(%rcx,%rax,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
lea    0x18(,%rax,8),%r14
mov    0x38(%rsp),%rbx
mov    %r12,%rdx
mov    0xc6c30(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    0x30(%rsp),%rbp
mov    0x20(%rsp),%rcx
lea    (%r15,%r14,8),%r14
lea    (%r14,%rbx,8),%rsi
lea    (%r14,%rcx,8),%rdi
call   *%r12
mov    0x40(%rsp),%rax
mov    %rax,(%r14,%rbx,8)
movq   $0x1,0x160(%rsp)
mov    %rbp,0x170(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
mov    %rbp,0x28(%rsp)
mov    $0x10,%r12d
mov    $0x1,%sil
lock add %r12,0x380(%rsp)
lock incq 0x388(%rsp)
lock incq 0x398(%rsp)
lea    -0x2a23d2(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%r12,%rax,1),%ebp
cmp    $0xff,%rbp
mov    %r8,0x40(%rsp)
mov    %rbx,0x60(%rsp)
mov    %rdx,0x30(%rsp)
mov    %esi,0x48(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1731>
cmp    $0x3d,%bpl
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a31>
mov    0x190(%rsp,%rbp,8),%r15
test   %r15,%r15
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x158c>
mov    (%r15),%rax
mov    %rax,0x190(%rsp,%rbp,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1721>
cmp    $0x18,%edx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x188b>
cmp    $0x10,%edx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18ab>
mov    %rbp,0x28(%rsp)
mov    $0xc0,%r12d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc69>
xor    %eax,%eax
cmp    $0x19,%rcx
setae  %al
lea    0x18(,%rax,8),%rcx
movl   $0x0,0x60(%rsp)
mov    %rbp,0x28(%rsp)
mov    %ecx,%ebx
shl    $0x4,%ebx
mov    0x188(%rsp),%eax
test   %eax,%eax
mov    %r13,0x90(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xda2>
lock add %rbx,0x380(%rsp)
lock incq 0x388(%rsp)
lock incq 0x398(%rsp)
cmp    $0x17,%ecx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf19>
lea    -0x2a24a7(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rbx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf19>
cmp    $0x3d,%dil
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a34>
mov    %rcx,%rdx
mov    0x190(%rsp,%rdi,8),%rcx
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe2b>
mov    (%rcx),%rax
mov    %rax,0x190(%rsp,%rdi,8)
mov    %rcx,%rax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf04>
mov    %rcx,0x20(%rsp)
mov    %rbx,0x48(%rsp)
mov    0x180(%rsp),%rbp
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x195f>
shl    $0x6,%rax
mov    0x48(%rsp),%rbx
lock add %rbx,0x0(%rbp,%rax,1)
lock incq 0x8(%rbp,%rax,1)
lock incq 0x10(%rbp,%rax,1)
cmpl   $0x17,0x20(%rsp)
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf19>
lea    -0x2a253d(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rbx,%rax,1),%esi
cmp    $0xff,%rsi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf19>
mov    0x1000(%rbp),%rdi
add    $0x40,%rdi
call   *0xc6c70(%rip)        # ADDR <_DYNAMIC+0x520>
test   %rax,%rax
mov    0x48(%rsp),%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf04>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf19>
cmp    $0x11,%edx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf19>
mov    %rbx,0x48(%rsp)
mov    %rdi,0x150(%rsp)
movq   $0x0,0xb0(%rsp)
mov    $0x40,%eax
mov    %rax,0x20(%rsp)
mov    $0x1000,%ebx
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xb0(%rsp),%rdi
call   *0xc69ec(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    0xb0(%rsp),%rbp
test   %rbp,%rbp
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    $0x1000,%edx
mov    %rbp,%rdi
xor    %esi,%esi
call   *0xc69d3(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x3a0(%rsp),%rax
mov    %rax,0x0(%rbp)
mov    0x150(%rsp),%rdi
mov    %rdi,0x8(%rbp)
mov    %rbp,0x3a0(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
mov    0x48(%rsp),%rbx
div    %bx
movzwl %ax,%eax
xchg   %ax,%ax
dec    %rax
mov    %rax,%rcx
imul   %rbx,%rcx
lea    (%rcx,%rbp,1),%rdx
add    $0x40,%rdx
mov    0x190(%rsp,%rdi,8),%rsi
mov    %rsi,0x40(%rbp,%rcx,1)
mov    %rdx,0x190(%rsp,%rdi,8)
cmp    $0x1,%rax
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xed0>
add    $0x40,%rbp
mov    %rbp,%rax
mov    %rax,0x20(%rsp)
mov    %rax,%rdi
xor    %esi,%esi
mov    %rbx,%rdx
call   *0xc6959(%rip)        # ADDR <memset@GLIBC_2.2.5>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf35>
mov    $0x1,%esi
mov    %rbx,%rdi
call   *0xc6941(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,0x20(%rsp)
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a7a>
mov    0x38(%rsp),%rbx
lea    0x0(,%rbx,8),%rdx
mov    0x20(%rsp),%r13
mov    %r13,%rdi
mov    %r15,%rsi
mov    %rdx,0x48(%rsp)
mov    0xc6827(%rip),%rbp        # ADDR <memcpy@GLIBC_2.14>
call   *%rbp
mov    %rbp,%rax
mov    0x28(%rsp),%rcx
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
cmpb   $0x0,0x60(%rsp)
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xfb6>
mov    0x30(%rsp),%rcx
cmp    $0x11,%rcx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xfa5>
lea    0x3(%rcx),%eax
and    $0x3c,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xfb6>
xor    %eax,%eax
cmp    $0x19,%rcx
setae  %al
lea    0x18(,%rax,8),%rax
mov    0x38(%rsp),%r13
mov    0x20(%rsp),%rcx
lea    (%rcx,%rax,8),%rbx
mov    %rbx,%rdi
mov    %r12,%rsi
mov    0x48(%rsp),%rdx
call   *0xc67ab(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x40(%rsp),%rax
mov    %rax,(%rbx,%r13,8)
lea    0x1(%r13),%rax
lea    (%rbx,%rax,8),%rdi
lea    (%r12,%r13,8),%rsi
mov    %rbp,%rdx
call   *0xc678d(%rip)        # ADDR <memcpy@GLIBC_2.14>
cmp    $0x3,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x101a>
cmp    $0x11,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1009>
add    $0x3,%r14d
and    $0x3c,%r14d
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x101a>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
lea    0x18(,%rax,8),%r14
mov    0x90(%rsp),%r13
mov    %r14,%rbx
shl    $0x4,%rbx
mov    0x188(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x108f>
lock sub %rbx,0x380(%rsp)
lock decq 0x388(%rsp)
cmp    $0x17,%r14
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x107f>
lea    -0x2a279a(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rbx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x107f>
cmp    $0x3e,%dil
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a2a>
mov    0x190(%rsp,%rdi,8),%rax
mov    %rax,(%r15)
mov    %r15,0x190(%rsp,%rdi,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1088>
mov    %r15,%rdi
call   *0xc6710(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x20(%rsp),%r12
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x10dc>
mov    0x180(%rsp),%r14
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
mov    0x20(%rsp),%r12
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x196a>
shl    $0x6,%rax
lock sub %rbx,(%r14,%rax,1)
lock decq 0x8(%r14,%rax,1)
mov    0x1000(%r14),%rdi
add    $0x40,%rdi
mov    $0x10,%ecx
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xc69b4(%rip)        # ADDR <_DYNAMIC+0x528>
movq   $0x1,0x160(%rsp)
mov    %r12,0x168(%rsp)
mov    0x30(%rsp),%rax
mov    %rax,0x170(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
mov    %rbx,%r12
mov    %rbp,0x28(%rsp)
mov    %r8,%rbp
movq   $0x0,0xb0(%rsp)
mov    $0x40,%eax
mov    %rax,0x20(%rsp)
mov    $0x1000,%ebx
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xb0(%rsp),%rdi
call   *0xc6720(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    0xb0(%rsp),%r15
test   %r15,%r15
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    $0x1000,%edx
mov    %r15,%rdi
xor    %esi,%esi
call   *0xc6707(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x3a0(%rsp),%rax
mov    %rax,(%r15)
movq   $0x6,0x8(%r15)
mov    %r15,0x3a0(%rsp)
lea    0xff0(%r15),%rax
mov    $0xffffffffffffff08,%rcx
mov    %r12,%rbx
data16 cs nopw 0x0(%rax,%rax,1)
mov    0x1c0(%rsp),%rdx
mov    %rdx,(%rax)
mov    %rax,0x1c0(%rsp)
lea    -0x10(%rax),%rdx
mov    0x1c0(%rsp),%rsi
mov    %rsi,-0x10(%rax)
mov    %rdx,0x1c0(%rsp)
lea    -0x20(%rax),%rdx
mov    0x1c0(%rsp),%rsi
mov    %rsi,-0x20(%rax)
mov    %rdx,0x1c0(%rsp)
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x120a>
lea    -0x30(%rax),%rdx
mov    0x1c0(%rsp),%rsi
mov    %rsi,-0x30(%rax)
mov    %rdx,0x1c0(%rsp)
add    $0xffffffffffffffc0,%rax
add    $0x4,%rcx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x11a0>
movq   $0x0,0x40(%r15)
add    $0x40,%r15
mov    %rbp,%r8
mov    0x28(%rsp),%rbp
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1837>
cmp    $0x10,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18e0>
cmp    $0x18,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18f3>
cs nopw 0x0(%rax,%rax,1)
mov    0x400(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x14ae>
mov    0x418(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1481>
movq   $0x0,0x418(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1481>
movzbl 0x3f9(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12b9>
mov    0x3b8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2a4b04(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1481>
movzbl 0x3fa(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1304>
mov    0x3c0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2a4b4f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1481>
movzbl 0x3fb(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x134f>
mov    0x3c8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2a4b9a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1481>
movzbl 0x3fc(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x139a>
mov    0x3d0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2a4be5(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1481>
movzbl 0x3fd(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x13e5>
mov    0x3d8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2a4c30(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1481>
movzbl 0x3fe(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1430>
mov    0x3e0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2a4c7b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1481>
movzbl 0x3ff(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1477>
mov    0x3e8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r9
lea    -0x2a4cc2(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
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
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a58>
movq   $0xffffffffffffffff,0x3f0(%rsp)
movw   $0x0,0x420(%rsp)
xorps  %xmm0,%xmm0
lea    0x3b0(%rsp),%rax
movups %xmm0,0x50(%rax)
movq   $0x0,0x60(%rax)
mov    0x188(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x14f0>
lea    0x3b0(%rsp),%rax
mov    %rax,(%rsp)
lea    0xb0(%rsp),%rdi
lea    0x180(%rsp),%rsi
lea    0x168(%rsp),%rdx
mov    %rbp,%rcx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1570>
movzbl 0x3a8(%rsp),%eax
test   %al,%al
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1539>
lea    0x3b0(%rsp),%rax
mov    %rax,(%rsp)
movq   $0x0,0x8(%rsp)
lea    0xb0(%rsp),%rdi
lea    0x180(%rsp),%rsi
lea    0x168(%rsp),%rdx
mov    %rbp,%rcx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1570>
lea    0x3b0(%rsp),%rax
mov    %rax,(%rsp)
lea    0xb0(%rsp),%rdi
lea    0x180(%rsp),%rsi
lea    0x168(%rsp),%rdx
mov    %rbp,%rcx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, true>>
nopl   0x0(%rax,%rax,1)
cmpq   $0x0,0xb0(%rsp)
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
incq   0x178(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
cmp    $0x101,%r12d
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1731>
movq   $0x0,0xb0(%rsp)
mov    $0x40,%eax
mov    %rax,0x20(%rsp)
mov    $0x1000,%ebx
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xb0(%rsp),%rdi
call   *0xc6294(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    0xb0(%rsp),%r15
test   %r15,%r15
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    $0x1000,%edx
mov    %r15,%rdi
xor    %esi,%esi
call   *0xc627b(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x3a0(%rsp),%rax
mov    %rax,(%r15)
mov    %rbp,0x8(%r15)
mov    %r15,0x3a0(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r12w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x165c>
mov    %rbp,%r9
mov    %rcx,%rax
nopl   0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rsi
imul   %r12,%rsi
lea    (%r15,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x190(%rsp,%r9,8),%r8
mov    %r8,0x40(%r15,%rsi,1)
mov    %rdi,0x190(%rsp,%r9,8)
dec    %rdx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1630>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
mov    0x60(%rsp),%rbx
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x171d>
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
mov    %r15,%r9
nopl   0x0(%rax)
lea    (%r9,%r8,1),%r10
mov    0x190(%rsp,%rbp,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x190(%rsp,%rbp,8)
lea    (%r9,%rdi,1),%r10
mov    0x190(%rsp,%rbp,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x190(%rsp,%rbp,8)
lea    (%r9,%rsi,1),%r10
mov    0x190(%rsp,%rbp,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x190(%rsp,%rbp,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x190(%rsp,%rbp,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x190(%rsp,%rbp,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x16b0>
add    $0x40,%r15
mov    %r15,%rdi
xor    %esi,%esi
mov    %r12,%rdx
call   *0xc6141(%rip)        # ADDR <memset@GLIBC_2.2.5>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x174b>
mov    $0x1,%esi
mov    %r12,%rdi
call   *0xc6129(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r15
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a6b>
mov    0x20(%rbx,%r14,8),%rbx
mov    0x38(%rsp),%rbp
lea    0x0(,%rbp,8),%r12d
mov    %r15,%rdi
mov    %rbx,%rsi
mov    %r12,%rdx
call   *0xc6014(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    (%r15,%r12,1),%rdi
add    $0x8,%rdi
add    %rbx,%r12
mov    0x30(%rsp),%rdx
sub    %rbp,%rdx
shl    $0x3,%rdx
mov    %r12,%rsi
call   *0xc5ff4(%rip)        # ADDR <memcpy@GLIBC_2.14>
test   %rbx,%rbx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a0f>
cmpb   $0x0,0x48(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17a3>
mov    0x30(%rsp),%rdx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17b3>
mov    0x30(%rsp),%rdx
cmp    $0x11,%edx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17c6>
add    $0x3,%edx
and    $0x3c,%edx
lea    0x0(,%rdx,8),%ecx
lea    0x8(,%rdx,8),%eax
and    $0xfffffff0,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17da>
xor    %eax,%eax
cmp    $0x19,%edx
setae  %al
shl    $0x6,%eax
add    $0xc0,%rax
mov    %rax,%rcx
lock sub %rax,0x380(%rsp)
lock decq 0x388(%rsp)
lea    -0x2a2f3a(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b6>
movzbl (%rcx,%rax,1),%edi
cmp    $0xff,%rdi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x181f>
cmp    $0x3e,%dil
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a2a>
mov    0x190(%rsp,%rdi,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0x190(%rsp,%rdi,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1828>
mov    %rbx,%rdi
call   *0xc5f70(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x40(%rsp),%r8
mov    0x28(%rsp),%rbp
mov    0x60(%rsp),%rbx
mov    0x38(%rsp),%rax
mov    %r8,(%r15,%rax,8)
mov    %r15,0x20(%rbx,%r14,8)
mov    $0x1,%eax
mov    %ebp,%ecx
shl    %cl,%rax
shr    $0x3,%ebp
and    $0x18,%ebp
or     %rax,(%rbx,%rbp,1)
incq   0x418(%rsp)
mov    0x420(%rsp),%eax
lea    0x1(%rax),%ecx
mov    %cx,0x420(%rsp)
incq   0x178(%rsp)
mov    0x3b0(%rsp),%rcx
mov    %al,0x8(%rcx)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
cmp    $0xffffffff,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1937>
cmp    $0x18,%edx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18ab>
mov    %rbp,0x28(%rsp)
mov    $0x100,%r12d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc69>
mov    %r8,%r15
mov    %rbx,%r12
mov    0x20(%rbx,%r14,8),%rbx
mov    0x38(%rsp),%r14
lea    (%rbx,%r14,8),%rsi
lea    (%rbx,%r14,8),%rdi
add    $0x8,%rdi
sub    %r14,%rdx
shl    $0x3,%rdx
call   *0xc5f7c(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %r15,(%rbx,%r14,8)
mov    %r12,%rbx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1845>
mov    $0x18,%esi
lea    0x3(%rax),%edi
and    $0x3c,%edi
cmp    %edi,%esi
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1240>
mov    %bpl,(%rdx,%rax,1)
mov    %r8,(%rcx,%rax,8)
mov    0x3b0(%rsp),%rcx
mov    0x8(%rcx),%rdx
mov    $0xffffffffffffff00,%rsi
and    %rsi,%rdx
or     %rax,%rdx
mov    %rdx,0x8(%rcx)
inc    %eax
mov    %ax,0x420(%rsp)
incq   0x418(%rsp)
incq   0x178(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf0>
mov    %rbp,0x28(%rsp)
mov    $0x120,%r12d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc69>
call   *0xc6149(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x2c3>
call   *0xc613e(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xab4>
call   *0xc6133(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xdc9>
call   *0xc6128(%rip)        # ADDR <_DYNAMIC+0x530>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x10b1>
mov    0x160(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x199b>
cmp    $0x1,%ebx
mov    0x58(%rsp),%r14
mov    0x50(%rsp),%rdi
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19a7>
mov    0x170(%rsp),%rbx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19af>
mov    0x58(%rsp),%r14
mov    0x50(%rsp),%rdi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19af>
mov    0x178(%rsp),%rbx
mov    %rbx,0x98(%rsp)
lea    0x98(%rsp),%rax
cmpq   $0x0,(%r14)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19cb>
call   *0xc5dcd(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x98(%rsp),%rax
add    $0x428,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
lea    -0x2a52e7(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0xc314d(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0xbc2d6(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x6f(%rsp),%rdx
mov    $0x11,%esi
call   *0xc5e26(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a92>
lea    -0x2a523e(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8d5>
lea    0xc31db(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7158>
mov    $0x6,%esi
call   *0xc5f10(%rip)        # ADDR <_DYNAMIC+0x3d0>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a92>
mov    $0x3e,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a48>
mov    %rbp,%rdi
mov    $0x3e,%esi
lea    0xbc2a8(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    %rax,0x88(%rsp)
mov    0x88(%rsp),%rdx
call   *0xc5d1a(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a92>
mov    $0x8,%edi
mov    $0x8,%esi
lea    0xc3387(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a40>
mov    $0x10,%eax
mov    %rax,0x20(%rsp)
mov    %r12,%rbx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a84>
mov    $0x10,%eax
mov    %rax,0x20(%rsp)
mov    0x20(%rsp),%rdi
mov    %rbx,%rsi
call   *0xc5d56(%rip)        # ADDR <_DYNAMIC+0x280>
ud2
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a9c>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a9c>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a9c>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a9c>
mov    %rax,%rbx
mov    0x58(%rsp),%r14
mov    0x50(%rsp),%r15
lea    0x160(%rsp),%rdi
call   ADDR <core::ptr::drop_glue::<expanse_trie::map::ExpanseMap>>
cmpq   $0x0,(%r14)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1ac5>
mov    %r15,%rdi
call   *0xc5cd3(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
call   *0xc5d2d(%rip)        # ADDR <_DYNAMIC+0x298>
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
