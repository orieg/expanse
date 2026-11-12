push   %rbp
push   %r15
push   %r14
push   %r13
push   %r12
push   %rbx
sub    $0x408,%rsp
mov    %rdi,%r14
lea    0x170(%rsp),%rdi
xor    %ebx,%ebx
mov    $0x1f0,%edx
xor    %esi,%esi
call   *0xc8e75(%rip)        # ADDR <memset@GLIBC_2.2.5>
xorps  %xmm0,%xmm0
movups %xmm0,0x390(%rsp)
movups %xmm0,0x3a0(%rsp)
movups %xmm0,0x3b0(%rsp)
movups %xmm0,0x3c0(%rsp)
movq   $0x0,0x140(%rsp)
movq   $0x0,0x158(%rsp)
movl   $0x3,0x168(%rsp)
movups %xmm0,0x360(%rsp)
movups %xmm0,0x370(%rsp)
movups %xmm0,0x379(%rsp)
movq   $0xffffffffffffffff,0x3d0(%rsp)
movups %xmm0,0x3d8(%rsp)
movups %xmm0,0x3e8(%rsp)
movups %xmm0,0x3f2(%rsp)
mov    0x8(%r14),%r13
mov    %r14,0x80(%rsp)
mov    0x10(%r14),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a63>
lea    0x160(%rsp),%rbp
lea    0x0(,%rax,8),%rax
add    %r13,%rax
mov    %rax,0x138(%rsp)
lea    0xbf200(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x218>
mov    %rax,0x78(%rsp)
mov    %r13,%r9
mov    %r13,0x30(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x117>
mov    0x40(%rsp),%rcx
mov    %rcx,(%r12,%rax,8)
mov    0x50(%rsp),%r9
add    $0x8,%r9
cmp    0x138(%rsp),%r9
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a3f>
mov    (%r9),%rax
mov    %rax,0x90(%rsp)
lea    0x90(%rsp),%rcx
not    %rax
mov    %rax,0x98(%rsp)
lea    0x98(%rsp),%rax
mov    0x90(%rsp),%r11
mov    0x98(%rsp),%r8
mov    0x140(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1f0>
cmp    $0x1,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x250>
mov    %r8,0x40(%rsp)
mov    %r9,0x50(%rsp)
mov    0x148(%rsp),%r15
mov    0x150(%rsp),%r14
cmp    $0x3,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x2f5>
lea    (%r15,%r14,8),%r12
test   %r14,%r14
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x511>
lea    -0x1(%r14),%rax
mov    %r14,%rcx
cmp    -0x8(%r15,%r14,8),%r11
jbe    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x3c4>
cmp    $0x1f,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x7c8>
cmp    $0x1,%r14
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x5b4>
mov    $0x2,%eax
mov    %r11,0x18(%rsp)
cmp    %r14,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x526>
mov    $0x1,%dl
mov    %edx,0x58(%rsp)
mov    %rcx,0x20(%rsp)
mov    %rax,%rbx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x612>
data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %r8,%r12
mov    %r9,%r15
mov    0x168(%rsp),%eax
mov    %r11,%r13
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x30b>
addq   $0x10,0x360(%rsp)
incq   0x368(%rsp)
incq   0x378(%rsp)
mov    0x1e0(%rsp),%r14
test   %r14,%r14
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x3db>
mov    (%r14),%rax
mov    %rax,0x1e0(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x4ce>
data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %r11,%rax
shr    $0x8,%rax
cmp    %rax,0x3d0(%rsp)
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1530>
mov    0x3e8(%rsp),%r12
test   %r12,%r12
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x574>
mov    %r8,0x40(%rsp)
mov    %r9,0x50(%rsp)
mov    %r11d,%r14d
shr    $0x5,%r14d
and    $0x7,%r14d
mov    %r11d,%ecx
and    $0x1f,%ecx
mov    (%r12,%r14,4),%eax
mov    $0x1,%edx
shl    %cl,%edx
bt     %ecx,%eax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x6a7>
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
mov    0x20(%r12,%r14,8),%rcx
shr    $0x18,%eax
mov    0x40(%rsp),%rdx
mov    %rdx,(%rcx,%rax,8)
mov    0x30(%rsp),%r13
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x100>
cmp    $0x11,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x390>
lea    0x3(%r14),%eax
and    $0x3c,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x3ae>
mov    0x160(%rsp),%rbx
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19cd>
shl    $0x6,%rax
lock addq $0x10,(%rbx,%rax,1)
lock incq 0x8(%rbx,%rax,1)
lock incq 0x10(%rbx,%rax,1)
mov    0x1000(%rbx),%rdi
add    $0x40,%rdi
mov    $0xe,%esi
call   *0xc8d6c(%rip)        # ADDR <_DYNAMIC+0x528>
mov    %rax,%r14
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x4ce>
mov    $0x10,%eax
mov    %rax,0x48(%rsp)
mov    $0x10,%edi
mov    $0x1,%esi
call   *0xc8b1e(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r14
mov    $0x10,%ebx
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x4d5>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    $0x18,%eax
cmp    $0x19,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x3ae>
mov    $0x20,%eax
cmp    $0x21,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x3ae>
lea    0x3(%r14),%rax
and    $0xfffffffffffffffc,%rax
lea    (%r15,%rax,8),%r12
lea    -0x1(%r14),%rax
mov    %r14,%rcx
cmp    -0x8(%r15,%r14,8),%r11
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1aa>
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf7>
cmp    $0x1,%r14
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x77f>
xor    %eax,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x7ad>
movq   $0x0,0xa0(%rsp)
mov    $0x40,%eax
mov    %rax,0x48(%rsp)
mov    $0x1000,%ebx
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa0(%rsp),%rdi
call   *0xc8a82(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    0xa0(%rsp),%r14
test   %r14,%r14
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    $0x1000,%edx
mov    %r14,%rdi
xor    %esi,%esi
call   *0xc8a69(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x380(%rsp),%rax
mov    %rax,(%r14)
movq   $0xe,0x8(%r14)
mov    %r14,0x380(%rsp)
lea    0xff0(%r14),%rax
mov    $0xffffffffffffff08,%rcx
mov    0x1e0(%rsp),%rdx
mov    %rdx,(%rax)
mov    %rax,0x1e0(%rsp)
lea    -0x10(%rax),%rdx
mov    0x1e0(%rsp),%rsi
mov    %rsi,-0x10(%rax)
mov    %rdx,0x1e0(%rsp)
lea    -0x20(%rax),%rdx
mov    0x1e0(%rsp),%rsi
mov    %rsi,-0x20(%rax)
mov    %rdx,0x1e0(%rsp)
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x4ca>
lea    -0x30(%rax),%rdx
mov    0x1e0(%rsp),%rsi
mov    %rsi,-0x30(%rax)
mov    %rdx,0x1e0(%rsp)
add    $0xffffffffffffffc0,%rax
add    $0x4,%rcx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x460>
add    $0x40,%r14
xorps  %xmm0,%xmm0
movups %xmm0,(%r14)
mov    %r13,(%r14)
mov    %r12,0x8(%r14)
movq   $0x1,0x140(%rsp)
mov    %r14,0x148(%rsp)
movq   $0x1,0x150(%rsp)
mov    0x30(%rsp),%r13
lea    0x160(%rsp),%rbp
mov    %r15,%r9
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x105>
mov    $0x1,%eax
xor    %ecx,%ecx
mov    %r11,0x18(%rsp)
cmp    %r14,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1d1>
lea    (%r15,%rcx,8),%rsi
lea    0x1(%rcx),%r13
lea    (%r15,%rcx,8),%rdi
add    $0x8,%rdi
sub    %rcx,%r14
shl    $0x3,%r14
mov    %r14,%rdx
mov    0xc8939(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    %rcx,%rbx
call   *%r12
mov    %r13,%rdi
mov    0x30(%rsp),%r13
mov    %rbx,%rcx
mov    0x18(%rsp),%rax
mov    %rax,(%r15,%rbx,8)
mov    $0x1,%ebx
mov    %r14,%rdx
mov    $0x1,%r14d
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xda2>
mov    0x3f0(%rsp),%rcx
test   %rcx,%rcx
mov    0x30(%rsp),%r13
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1530>
movzwl 0x400(%rsp),%eax
mov    %rax,%rdx
cmp    $0x3,%rax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd26>
cmp    $0x11,%ax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd0a>
lea    0x3(%rax),%edx
and    $0x3c,%edx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd26>
lea    0x1(%r14),%rax
mov    %rax,0x38(%rsp)
cmp    $0x10,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa63>
mov    %rcx,0x20(%rsp)
mov    $0x3,%eax
cmp    $0x3,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x5ff>
lea    0x4(%r14),%eax
and    $0x1c,%eax
lea    0x3(%r14),%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xcbb>
mov    0x38(%rsp),%rcx
cmp    $0x11,%rcx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x138c>
mov    %rcx,%rax
mov    %r11,0x18(%rsp)
lea    0x3(%rax),%ebx
and    $0x3c,%ebx
movl   $0x0,0x58(%rsp)
mov    %rax,%rcx
mov    %rcx,0x38(%rsp)
mov    %ebx,%ebp
shl    $0x4,%ebp
mov    0x168(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x9aa>
mov    %rbp,0x28(%rsp)
cmp    $0x17,%ebx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa2c>
lea    -0x2a6b5b(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
mov    0x28(%rsp),%rcx
movzbl (%rcx,%rax,1),%ebp
cmp    $0xff,%rbp
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa2c>
mov    0x28(%rsp),%rax
add    %rax,0x360(%rsp)
incq   0x368(%rsp)
incq   0x378(%rsp)
cmp    $0x3e,%bpl
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1af0>
mov    0x170(%rsp,%rbp,8),%r13
test   %r13,%r13
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xe47>
mov    0x0(%r13),%rax
mov    %rax,0x170(%rsp,%rbp,8)
mov    0x28(%rsp),%rbp
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf14>
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
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa9f>
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
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xdd6>
cmp    $0x10,%edx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x10ea>
mov    $0x20,%r13d
cmp    $0x2,%edx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xddc>
lea    0x4(%rdx),%eax
lea    0x3(%rdx),%ecx
xor    %eax,%ecx
cmp    $0x4,%ecx
mov    0xc871d(%rip),%rcx        # ADDR <memmove@GLIBC_2.2.5>
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18dd>
lea    0x20(,%rdx,8),%r13d
and    $0xe0,%r13d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xddc>
mov    %r14,%rcx
xor    %eax,%eax
data16 data16 cs nopw 0x0(%rax,%rax,1)
mov    %rax,%rdx
mov    %rcx,%rsi
shr    $1,%rsi
add    %rsi,%rax
cmp    %r11,(%r15,%rax,8)
cmova  %rdx,%rax
sub    %rsi,%rcx
cmp    $0x1,%rcx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x790>
cmp    %r11,(%r15,%rax,8)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf7>
adc    $0x0,%rax
mov    %rax,%rcx
cmp    $0x1f,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4>
mov    0x168(%rsp),%eax
test   %eax,%eax
mov    %r11,0x18(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x8a0>
xorps  %xmm0,%xmm0
movaps %xmm0,0x60(%rsp)
lea    0x0(,%r14,8),%rbx
xor    %r13d,%r13d
nop
mov    (%r15,%r13,1),%rcx
mov    (%r12,%r13,1),%r8
xorps  %xmm0,%xmm0
movaps %xmm0,0xd0(%rsp)
movaps %xmm0,0xc0(%rsp)
movaps %xmm0,0xb0(%rsp)
movaps %xmm0,0xa0(%rsp)
movq   $0xffffffffffffffff,0xe0(%rsp)
lea    0xe8(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0xa0(%rsp),%rax
mov    %rax,(%rsp)
lea    0x120(%rsp),%rdi
mov    %rbp,%rsi
lea    0x60(%rsp),%rdx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
add    $0x8,%r13
cmp    %r13,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x7f0>
lea    0x390(%rsp),%rax
mov    %rax,(%rsp)
lea    0xa0(%rsp),%rdi
mov    %rbp,%rsi
lea    0x60(%rsp),%rdx
mov    0x18(%rsp),%rcx
mov    0x40(%rsp),%r8
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb9b>
movzbl 0x388(%rsp),%eax
xorps  %xmm0,%xmm0
test   %al,%al
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xae0>
movaps %xmm0,0x60(%rsp)
movl   $0x0,0x74(%rsp)
lea    0x0(,%r14,8),%rbx
xor    %r13d,%r13d
nopl   0x0(%rax,%rax,1)
mov    (%r15,%r13,1),%rcx
mov    (%r12,%r13,1),%r8
xorps  %xmm0,%xmm0
movaps %xmm0,0xd0(%rsp)
movaps %xmm0,0xc0(%rsp)
movaps %xmm0,0xb0(%rsp)
movaps %xmm0,0xa0(%rsp)
movq   $0xffffffffffffffff,0xe0(%rsp)
lea    0xe8(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0x74(%rsp),%rax
mov    %rax,0x10(%rsp)
lea    0xa0(%rsp),%rax
mov    %rax,(%rsp)
movq   $0x1,0x8(%rsp)
lea    0x120(%rsp),%rdi
mov    %rbp,%rsi
lea    0x60(%rsp),%rdx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, false>>
add    $0x8,%r13
cmp    %r13,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x8d0>
lea    0x74(%rsp),%rax
mov    %rax,0x10(%rsp)
lea    0x390(%rsp),%rax
mov    %rax,(%rsp)
movq   $0x1,0x8(%rsp)
lea    0xa0(%rsp),%rdi
mov    %rbp,%rsi
lea    0x60(%rsp),%rdx
mov    0x18(%rsp),%rcx
mov    0x40(%rsp),%r8
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xb9b>
mov    0x160(%rsp),%r13
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19e3>
shl    $0x6,%rax
lock add %rbp,0x0(%r13,%rax,1)
lock incq 0x8(%r13,%rax,1)
lock incq 0x10(%r13,%rax,1)
cmp    $0x17,%ebx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa14>
lea    -0x2a6f01(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl 0x0(%rbp,%rax,1),%esi
cmp    $0xff,%rsi
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa14>
mov    0x1000(%r13),%rdi
add    $0x40,%rdi
call   *0xc86b8(%rip)        # ADDR <_DYNAMIC+0x528>
mov    %rax,%r13
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf14>
mov    $0x1,%esi
mov    %rbp,%rdi
call   *0xc8476(%rip)        # ADDR <calloc@GLIBC_2.2.5>
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa5b>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b3f>
lea    0x160(%rsp),%rdi
mov    0x28(%rsp),%rsi
call   *0xc8659(%rip)        # ADDR <_DYNAMIC+0x500>
mov    $0x1,%esi
mov    0x28(%rsp),%rbx
mov    %rbx,%rdi
call   *0xc8446(%rip)        # ADDR <calloc@GLIBC_2.2.5>
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b33>
mov    %rax,%r13
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf22>
cmp    $0x18,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc9b>
cmp    $0x10,%r14
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xcc0>
mov    %rcx,0x20(%rsp)
mov    $0x18,%eax
mov    $0x11,%ecx
mov    %rcx,0x38(%rsp)
lea    0x3(%r14),%ecx
and    $0x3c,%ecx
cmp    %eax,%ecx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x5ed>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xcbb>
addq   $0x10,0x360(%rsp)
incq   0x368(%rsp)
incq   0x378(%rsp)
mov    0x1a0(%rsp),%r15
test   %r15,%r15
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x13af>
mov    (%r15),%rax
mov    %rax,0x1a0(%rsp)
movq   $0x0,(%r15)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x14b9>
movaps %xmm0,0x60(%rsp)
lea    0x0(,%r14,8),%rbx
xor    %r13d,%r13d
mov    (%r15,%r13,1),%rcx
mov    (%r12,%r13,1),%r8
xorps  %xmm0,%xmm0
movaps %xmm0,0xd0(%rsp)
movaps %xmm0,0xc0(%rsp)
movaps %xmm0,0xb0(%rsp)
movaps %xmm0,0xa0(%rsp)
movq   $0xffffffffffffffff,0xe0(%rsp)
lea    0xe8(%rsp),%rax
movups %xmm0,0x1a(%rax)
movups %xmm0,0x10(%rax)
movups %xmm0,(%rax)
lea    0xa0(%rsp),%rax
mov    %rax,(%rsp)
lea    0x120(%rsp),%rdi
mov    %rbp,%rsi
lea    0x60(%rsp),%rdx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, true>>
add    $0x8,%r13
cmp    %r13,%rbx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xaf0>
lea    0x390(%rsp),%rax
mov    %rax,(%rsp)
lea    0xa0(%rsp),%rdi
mov    %rbp,%rsi
lea    0x60(%rsp),%rdx
mov    0x18(%rsp),%rcx
mov    0x40(%rsp),%r8
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, true>>
movaps 0x60(%rsp),%xmm0
movaps %xmm0,0x120(%rsp)
mov    %r14,%rbx
shl    $0x4,%rbx
add    $0x30,%rbx
and    $0xffffffffffffffc0,%rbx
cmp    $0x21,%r14
mov    $0x200,%eax
cmovb  %rax,%rbx
mov    0x168(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc23>
cmp    $0x177,%rbx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1918>
lea    -0x2a70fb(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rbx,%rax,1),%ebp
cmp    $0xff,%rbp
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a29>
sub    %rbx,0x360(%rsp)
decq   0x368(%rsp)
cmp    $0x3e,%bpl
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1ae9>
mov    0x170(%rsp,%rbp,8),%rax
mov    %rax,(%r15)
mov    %r15,0x170(%rsp,%rbp,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc6c>
mov    0x160(%rsp),%r12
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19d8>
shl    $0x6,%rax
lock sub %rbx,(%r12,%rax,1)
lock decq 0x8(%r12,%rax,1)
mov    0x1000(%r12),%rdi
add    $0x40,%rdi
mov    $0x10,%ecx
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xc845c(%rip)        # ADDR <_DYNAMIC+0x530>
inc    %r14
mov    %r14,0x158(%rsp)
movaps 0x120(%rsp),%xmm0
lea    0x148(%rsp),%rax
movups %xmm0,(%rax)
movq   $0x2,0x140(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x10d8>
mov    %rcx,0x20(%rsp)
setne  %al
movzbl %al,%eax
lea    0x18(,%rax,8),%rcx
mov    $0x20,%eax
cmp    %eax,%ecx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x5ed>
mov    0x20(%rsp),%rcx
lea    (%r15,%rcx,8),%rsi
mov    %r11,%r12
lea    0x1(%rcx),%rax
mov    %rax,0x28(%rsp)
lea    (%r15,%rcx,8),%rdi
add    $0x8,%rdi
mov    %r14,%rdx
sub    %rcx,%rdx
shl    $0x3,%rdx
mov    %rdx,0x20(%rsp)
mov    %rcx,%rbx
call   *0xc8190(%rip)        # ADDR <memmove@GLIBC_2.2.5>
mov    %rbx,%rcx
mov    %r12,(%r15,%rbx,8)
cmp    $0x11,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd7b>
add    $0x3,%r14d
and    $0x3c,%r14d
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd8c>
mov    $0x18,%edx
cmp    $0x19,%ax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd26>
mov    $0x20,%edx
cmp    $0x21,%ax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd26>
lea    0x3(%rax),%edx
and    $0xfffffffc,%edx
lea    (%rcx,%rdx,8),%rdx
cmp    %r11b,-0x1(%rax,%rdx,1)
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xd6b>
lea    -0x19(%rax),%esi
cmp    $0xffe9,%si
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1530>
cmp    $0x10,%ax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x151a>
cmp    $0x3,%ax
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1530>
lea    0x4(%rax),%esi
and    $0x1c,%esi
lea    0x3(%rax),%edi
and    $0x3c,%edi
cmp    %edi,%esi
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1530>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x197c>
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1530>
mov    %r8,-0x8(%rcx,%rax,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x105>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
lea    0x18(,%rax,8),%r14
mov    0x20(%rsp),%rdx
mov    0xc80e8(%rip),%r12        # ADDR <memmove@GLIBC_2.2.5>
mov    0x38(%rsp),%rbx
mov    0x28(%rsp),%rdi
lea    (%r15,%r14,8),%r14
lea    (%r14,%rcx,8),%rsi
lea    (%r14,%rdi,8),%rdi
mov    %rcx,%r15
call   *%r12
mov    0x40(%rsp),%rax
mov    %rax,(%r14,%r15,8)
movq   $0x1,0x140(%rsp)
mov    %rbx,0x150(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x100>
mov    $0x10,%r13d
mov    %rdx,0x20(%rsp)
lea    -0x2a7300(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl 0x0(%r13,%rax,1),%ebp
cmp    $0xff,%rbp
mov    %r11,0x18(%rsp)
mov    %esi,0x28(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19f9>
add    %r13,0x360(%rsp)
incq   0x368(%rsp)
incq   0x378(%rsp)
cmp    $0x3e,%bpl
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1af0>
mov    0x170(%rsp,%rbp,8),%r15
test   %r15,%r15
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1110>
mov    (%r15),%rax
mov    %rax,0x170(%rsp,%rbp,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12a1>
cmp    $0x11,%ebx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xa3f>
movq   $0x0,0xa0(%rsp)
mov    $0x40,%eax
mov    %rax,0x48(%rsp)
mov    $0x1000,%ebx
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa0(%rsp),%rdi
call   *0xc800d(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    0xa0(%rsp),%r13
test   %r13,%r13
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    $0x1000,%edx
mov    %r13,%rdi
xor    %esi,%esi
call   *0xc7ff4(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x380(%rsp),%rax
mov    %rax,0x0(%r13)
mov    %rbp,0x8(%r13)
mov    %r13,0x380(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
mov    0x28(%rsp),%rdi
div    %di
movzwl %ax,%eax
data16 cs nopw 0x0(%rax,%rax,1)
dec    %rax
mov    %rax,%rcx
imul   %rdi,%rcx
lea    (%rcx,%r13,1),%rdx
add    $0x40,%rdx
mov    0x170(%rsp,%rbp,8),%rsi
mov    %rsi,0x40(%r13,%rcx,1)
mov    %rdx,0x170(%rsp,%rbp,8)
cmp    $0x1,%rax
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xee0>
add    $0x40,%r13
mov    %rdi,%rbp
mov    %r13,%rdi
xor    %esi,%esi
mov    %rbp,%rdx
call   *0xc7f7e(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x20(%rsp),%rbx
lea    0x0(,%rbx,8),%rdx
mov    %r13,%rdi
mov    %r15,%rsi
mov    %rdx,0x28(%rsp)
mov    0xc7e6f(%rip),%rbp        # ADDR <memcpy@GLIBC_2.14>
call   *%rbp
mov    %rbp,%rcx
mov    0x18(%rsp),%rax
mov    %rax,0x0(%r13,%rbx,8)
mov    %r13,0x18(%rsp)
lea    0x8(,%rbx,8),%rdi
add    %r13,%rdi
lea    (%r15,%rbx,8),%rsi
mov    %r14,%rbp
sub    %rbx,%rbp
shl    $0x3,%rbp
mov    %rbp,%rdx
call   *%rcx
mov    0x38(%rsp),%rax
cmpb   $0x0,0x58(%rsp)
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xfa3>
mov    0x38(%rsp),%rcx
cmp    $0x11,%rcx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xf92>
lea    0x3(%rcx),%eax
and    $0x3c,%eax
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xfa3>
xor    %eax,%eax
cmp    $0x19,%rcx
setae  %al
lea    0x18(,%rax,8),%rax
mov    0x20(%rsp),%r13
mov    0x18(%rsp),%rcx
lea    (%rcx,%rax,8),%rbx
mov    %rbx,%rdi
mov    %r12,%rsi
mov    0x28(%rsp),%rdx
call   *0xc7dee(%rip)        # ADDR <memcpy@GLIBC_2.14>
mov    0x40(%rsp),%rax
mov    %rax,(%rbx,%r13,8)
lea    0x1(%r13),%rax
lea    (%rbx,%rax,8),%rdi
lea    (%r12,%r13,8),%rsi
mov    %rbp,%rdx
call   *0xc7dd0(%rip)        # ADDR <memcpy@GLIBC_2.14>
cmp    $0x3,%r14
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1007>
cmp    $0x11,%r14
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xff6>
add    $0x3,%r14d
and    $0x3c,%r14d
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1007>
xor    %eax,%eax
cmp    $0x19,%r14
setae  %al
lea    0x18(,%rax,8),%r14
mov    0x18(%rsp),%r12
mov    %r14,%rbx
shl    $0x4,%rbx
mov    0x168(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x106f>
cmp    $0x17,%r14
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x194a>
lea    -0x2a7547(%rip),%rax        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rbx,%rax,1),%ebp
cmp    $0xff,%rbp
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x194a>
sub    %rbx,0x360(%rsp)
decq   0x368(%rsp)
cmp    $0x3e,%bpl
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1ae9>
mov    0x170(%rsp,%rbp,8),%rax
mov    %rax,(%r15)
mov    %r15,0x170(%rsp,%rbp,8)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x10b7>
mov    0x160(%rsp),%r14
mov    $0xffffffffffffff10,%rax
mov    %fs:(%rax),%rax
cmp    $0x40,%rax
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19ee>
shl    $0x6,%rax
lock sub %rbx,(%r14,%rax,1)
lock decq 0x8(%r14,%rax,1)
mov    0x1000(%r14),%rdi
add    $0x40,%rdi
mov    $0x10,%ecx
mov    %r15,%rsi
mov    %rbx,%rdx
call   *0xc8011(%rip)        # ADDR <_DYNAMIC+0x530>
movq   $0x1,0x140(%rsp)
mov    %r12,0x148(%rsp)
mov    0x38(%rsp),%rax
mov    %rax,0x150(%rsp)
mov    0x30(%rsp),%r13
lea    0x160(%rsp),%rbp
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x100>
cmp    $0x18,%edx
mov    0xc7d8c(%rip),%rcx        # ADDR <memmove@GLIBC_2.2.5>
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18c2>
cmp    $0x10,%edx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18dd>
mov    $0xc0,%r13d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xddc>
cmp    $0x101,%r13d
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a0a>
movq   $0x0,0xa0(%rsp)
mov    $0x40,%eax
mov    %rax,0x48(%rsp)
mov    $0x1000,%ebx
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa0(%rsp),%rdi
call   *0xc7d40(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    0xa0(%rsp),%r15
test   %r15,%r15
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    $0x1000,%edx
mov    %r15,%rdi
xor    %esi,%esi
call   *0xc7d27(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x380(%rsp),%rax
mov    %rax,(%r15)
mov    %rbp,0x8(%r15)
mov    %r15,0x380(%rsp)
mov    $0xfc0,%ax
xor    %edx,%edx
div    %r13w
movzwl %ax,%ecx
mov    %ecx,%edx
dec    %edx
mov    %rcx,%rax
and    $0x3,%edx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x11dc>
mov    %rcx,%rax
nopl   0x0(%rax)
dec    %rax
mov    %rax,%rsi
imul   %r13,%rsi
lea    (%r15,%rsi,1),%rdi
add    $0x40,%rdi
mov    0x170(%rsp,%rbp,8),%r8
mov    %r8,0x40(%r15,%rsi,1)
mov    %rdi,0x170(%rsp,%rbp,8)
dec    %rdx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x11b0>
add    $0xfffffffe,%ecx
cmp    $0x3,%cx
jb     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x129d>
lea    -0x4(%rax),%rcx
imul   %r13,%rcx
add    $0x40,%rcx
lea    0x0(,%r13,4),%edx
neg    %rdx
lea    -0x3(%rax),%rsi
imul   %r13,%rsi
add    $0x40,%rsi
lea    -0x2(%rax),%rdi
imul   %r13,%rdi
add    $0x40,%rdi
lea    -0x1(%rax),%r8
imul   %r13,%r8
add    $0x40,%r8
mov    %r15,%r9
nopw   0x0(%rax,%rax,1)
lea    (%r9,%r8,1),%r10
mov    0x170(%rsp,%rbp,8),%r11
mov    %r11,(%r9,%r8,1)
mov    %r10,0x170(%rsp,%rbp,8)
lea    (%r9,%rdi,1),%r10
mov    0x170(%rsp,%rbp,8),%r11
mov    %r11,(%r9,%rdi,1)
mov    %r10,0x170(%rsp,%rbp,8)
lea    (%r9,%rsi,1),%r10
mov    0x170(%rsp,%rbp,8),%r11
mov    %r11,(%r9,%rsi,1)
mov    %r10,0x170(%rsp,%rbp,8)
add    $0xfffffffffffffffc,%rax
lea    (%r9,%rcx,1),%r10
mov    0x170(%rsp,%rbp,8),%r11
mov    %r11,(%r9,%rcx,1)
mov    %r10,0x170(%rsp,%rbp,8)
add    %rdx,%r9
cmp    $0x1,%rax
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1230>
add    $0x40,%r15
mov    %r15,%rdi
xor    %esi,%esi
mov    %r13,%rdx
call   *0xc7bf1(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    %r12,%r13
mov    0x20(%r12,%r14,8),%rbx
mov    0x38(%rsp),%rbp
lea    0x0(,%rbp,8),%r12d
mov    %r15,%rdi
mov    %rbx,%rsi
mov    %r12,%rdx
call   *0xc7add(%rip)        # ADDR <memcpy@GLIBC_2.14>
lea    (%r15,%r12,1),%rdi
add    $0x8,%rdi
add    %rbx,%r12
mov    0x20(%rsp),%rdx
sub    %rbp,%rdx
shl    $0x3,%rdx
mov    %r12,%rsi
call   *0xc7abd(%rip)        # ADDR <memcpy@GLIBC_2.14>
test   %rbx,%rbx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1aa4>
cmpb   $0x0,0x28(%rsp)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x130d>
mov    0x20(%rsp),%rcx
mov    %r13,%r12
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1324>
mov    0x20(%rsp),%rcx
cmp    $0x11,%ecx
mov    %r13,%r12
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1878>
add    $0x3,%ecx
and    $0x3c,%ecx
lea    0x0(,%rcx,8),%eax
lea    0x8(,%rcx,8),%esi
and    $0xfffffff0,%esi
lea    -0x2a7854(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rax,%rcx,1),%ebp
cmp    $0xff,%rbp
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18a6>
sub    %rsi,0x360(%rsp)
decq   0x368(%rsp)
cmp    $0x3e,%bpl
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1ae9>
mov    0x170(%rsp,%rbp,8),%rax
mov    %rax,(%rbx)
mov    %rbx,0x170(%rsp,%rbp,8)
lea    0x160(%rsp),%rbp
mov    0x18(%rsp),%r11
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x14b9>
mov    %r11,0x18(%rsp)
xor    %eax,%eax
cmp    $0x19,%rcx
setae  %al
lea    0x18(,%rax,8),%rbx
movl   $0x0,0x58(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x615>
mov    %r11,%r13
movq   $0x0,0xa0(%rsp)
mov    $0x40,%eax
mov    %rax,0x48(%rsp)
mov    $0x1000,%ebx
mov    $0x40,%esi
mov    $0x1000,%edx
lea    0xa0(%rsp),%rdi
call   *0xc7aab(%rip)        # ADDR <posix_memalign@GLIBC_2.2.5>
test   %eax,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    0xa0(%rsp),%r15
test   %r15,%r15
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    $0x1000,%edx
mov    %r15,%rdi
xor    %esi,%esi
call   *0xc7a92(%rip)        # ADDR <memset@GLIBC_2.2.5>
mov    0x380(%rsp),%rax
mov    %rax,(%r15)
movq   $0x6,0x8(%r15)
mov    %r15,0x380(%rsp)
lea    0xff0(%r15),%rax
mov    $0xffffffffffffff08,%rcx
lea    0x160(%rsp),%rbp
nop
mov    0x1a0(%rsp),%rdx
mov    %rdx,(%rax)
mov    %rax,0x1a0(%rsp)
lea    -0x10(%rax),%rdx
mov    0x1a0(%rsp),%rsi
mov    %rsi,-0x10(%rax)
mov    %rdx,0x1a0(%rsp)
lea    -0x20(%rax),%rdx
mov    0x1a0(%rsp),%rsi
mov    %rsi,-0x20(%rax)
mov    %rdx,0x1a0(%rsp)
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x14aa>
lea    -0x30(%rax),%rdx
mov    0x1a0(%rsp),%rsi
mov    %rsi,-0x30(%rax)
mov    %rdx,0x1a0(%rsp)
add    $0xffffffffffffffc0,%rax
add    $0x4,%rcx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1440>
movq   $0x0,0x40(%r15)
add    $0x40,%r15
mov    %r13,%r11
mov    0x40(%rsp),%rax
mov    0x38(%rsp),%rcx
mov    %rax,(%r15,%rcx,8)
mov    %r15,0x20(%r12,%r14,8)
mov    $0x1,%eax
mov    %r11d,%ecx
shl    %cl,%rax
shr    $0x3,%r11d
and    $0x18,%r11d
or     %rax,(%r12,%r11,1)
incq   0x3f8(%rsp)
mov    0x400(%rsp),%eax
lea    0x1(%rax),%ecx
mov    %cx,0x400(%rsp)
incq   0x158(%rsp)
mov    0x390(%rsp),%rcx
mov    %al,0x8(%rcx)
mov    0x30(%rsp),%r13
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x100>
cmp    $0x10,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1969>
cmp    $0x18,%eax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x197c>
nopl   (%rax)
mov    0x3e0(%rsp),%rax
test   %rax,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x179e>
mov    0x3f8(%rsp),%rcx
test   %rcx,%rcx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1771>
movq   $0x0,0x3f8(%rsp)
cmp    $0x1,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1771>
movzbl 0x3d9(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x15a9>
mov    0x398(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r10
lea    -0x2a9bc4(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r10,%r8
cmp    $0x2,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1771>
movzbl 0x3da(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x15f4>
mov    0x3a0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r10
lea    -0x2a9c0f(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r10,%r8
cmp    $0x3,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1771>
movzbl 0x3db(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x163f>
mov    0x3a8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r10
lea    -0x2a9c5a(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r10,%r8
cmp    $0x4,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1771>
movzbl 0x3dc(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x168a>
mov    0x3b0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r10
lea    -0x2a9ca5(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r10,%r8
cmp    $0x5,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1771>
movzbl 0x3dd(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x16d5>
mov    0x3b8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r10
lea    -0x2a9cf0(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r10,%r8
cmp    $0x6,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1771>
movzbl 0x3de(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1720>
mov    0x3c0(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r10
lea    -0x2a9d3b(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r10,%r8
cmp    $0x7,%rax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1771>
movzbl 0x3df(%rsp),%edx
cmp    $0x7,%rdx
ja     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1767>
mov    0x3c8(%rsp),%rsi
mov    0x8(%rsi),%rdi
mov    %r8,%r10
lea    -0x2a9d82(%rip),%r8        # ADDR <anon.ADDR.24.llvm.ADDR+0xc895>
mov    (%r8,%rdx,8),%rdx
mov    %rdx,%r8
and    %rdi,%r8
add    %rcx,%r8
xor    %rdi,%r8
and    %rdx,%r8
xor    %rdi,%r8
mov    %r8,0x8(%rsi)
mov    %r10,%r8
cmp    $0x8,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b11>
movq   $0xffffffffffffffff,0x3d0(%rsp)
movw   $0x0,0x400(%rsp)
xorps  %xmm0,%xmm0
lea    0x390(%rsp),%rax
movups %xmm0,0x50(%rax)
movq   $0x0,0x60(%rax)
mov    0x168(%rsp),%eax
test   %eax,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x17db>
mov    %r9,%rbx
lea    0x390(%rsp),%rax
mov    %rax,(%rsp)
lea    0xa0(%rsp),%rdi
mov    %rbp,%rsi
lea    0x148(%rsp),%rdx
mov    %r11,%rcx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_flat::<false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1850>
mov    %r9,%rbx
movzbl 0x388(%rsp),%eax
test   %al,%al
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1822>
lea    0x390(%rsp),%rax
mov    %rax,(%rsp)
movq   $0x0,0x8(%rsp)
lea    0xa0(%rsp),%rdi
mov    %rbp,%rsi
lea    0x148(%rsp),%rdx
mov    %r11,%rcx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, false>>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1850>
lea    0x390(%rsp),%rax
mov    %rax,(%rsp)
lea    0xa0(%rsp),%rdi
mov    %rbp,%rsi
lea    0x148(%rsp),%rdx
mov    %r11,%rcx
mov    $0x8,%r9d
call   ADDR <expanse_trie::mutate_map::map_insert_with_path_occ::<false, true, true>>
nop
cmpq   $0x0,0xa0(%rsp)
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1863>
incq   0x158(%rsp)
mov    0x30(%rsp),%r13
lea    0x160(%rsp),%rbp
mov    %rbx,%r9
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x105>
xor    %esi,%esi
cmp    $0x19,%ecx
setae  %sil
shl    $0x6,%esi
add    $0xc0,%rsi
mov    %rsi,%rax
lea    -0x2a7dad(%rip),%rcx        # ADDR <anon.ADDR.24.llvm.ADDR+0xe9b5>
movzbl (%rax,%rcx,1),%ebp
cmp    $0xff,%rbp
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x134d>
lea    0x160(%rsp),%rdi
call   *0xc781c(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %rbx,%rdi
call   *0xc750b(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x137a>
cmp    $0xffffffff,%eax
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x19c0>
cmp    $0x18,%edx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x18dd>
mov    $0x100,%r13d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xddc>
mov    %r11,%r15
mov    0x20(%r12,%r14,8),%rbx
mov    0x38(%rsp),%r14
lea    (%rbx,%r14,8),%rsi
lea    (%rbx,%r14,8),%rdi
add    $0x8,%rdi
sub    %r14,%rdx
shl    $0x3,%rdx
call   *%rcx
mov    0x40(%rsp),%rax
mov    %rax,(%rbx,%r14,8)
mov    %r15,%r11
lea    0x160(%rsp),%rbp
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x14cc>
lea    0x160(%rsp),%rdi
mov    %rbx,%rsi
call   *0xc77a7(%rip)        # ADDR <_DYNAMIC+0x538>
movabs $0x7ffffffffffffff1,%rax
cmp    %rax,%rbx
jae    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1ac2>
mov    %r15,%rdi
call   *0xc7483(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc6c>
lea    0x160(%rsp),%rdi
mov    %rbx,%rsi
call   *0xc7775(%rip)        # ADDR <_DYNAMIC+0x538>
mov    %r15,%rdi
call   *0xc7464(%rip)        # ADDR <free@GLIBC_2.2.5>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x10b7>
mov    $0x18,%esi
lea    0x3(%rax),%edi
and    $0x3c,%edi
cmp    %edi,%esi
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1530>
mov    %r11b,(%rdx,%rax,1)
mov    %r8,(%rcx,%rax,8)
mov    0x390(%rsp),%rcx
mov    0x8(%rcx),%rdx
mov    $0xffffffffffffff00,%rsi
and    %rsi,%rdx
or     %rax,%rdx
mov    %rdx,0x8(%rcx)
inc    %eax
mov    %ax,0x400(%rsp)
incq   0x3f8(%rsp)
incq   0x158(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x105>
mov    $0x120,%r13d
xor    %esi,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xddc>
call   *0xc7705(%rip)        # ADDR <_DYNAMIC+0x540>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x328>
call   *0xc76fa(%rip)        # ADDR <_DYNAMIC+0x540>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0xc40>
call   *0xc76ef(%rip)        # ADDR <_DYNAMIC+0x540>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x9c7>
call   *0xc76e4(%rip)        # ADDR <_DYNAMIC+0x540>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x108c>
lea    0x160(%rsp),%rdi
mov    %r13,%rsi
call   *0xc768e(%rip)        # ADDR <_DYNAMIC+0x500>
mov    $0x1,%esi
mov    %r13,%rdi
call   *0xc7480(%rip)        # ADDR <calloc@GLIBC_2.2.5>
mov    %rax,%r15
test   %rax,%rax
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x12af>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b24>
lea    0x160(%rsp),%rdi
mov    %rbx,%rsi
call   *0xc7696(%rip)        # ADDR <_DYNAMIC+0x538>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x193c>
mov    0x140(%rsp),%rbx
test   %rbx,%rbx
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a63>
cmp    $0x1,%ebx
jne    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a5b>
mov    0x150(%rsp),%rbx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a63>
mov    0x158(%rsp),%rbx
mov    %rbx,0x88(%rsp)
lea    0x88(%rsp),%rax
mov    0x80(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1a8a>
mov    %r13,%rdi
call   *0xc733e(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    0x88(%rsp),%rax
add    $0x408,%rsp
pop    %rbx
pop    %r12
pop    %r13
pop    %r14
pop    %r15
pop    %rbp
ret
lea    -0x2aa0a3(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc8d5>
lea    0xc4776(%rip),%rdx        # ADDR <anon.ADDR.2.llvm.ADDR+0x7158>
mov    $0x6,%esi
call   *0xc74ab(%rip)        # ADDR <_DYNAMIC+0x3d0>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b5a>
lea    -0x2aa194(%rip),%rdi        # ADDR <anon.ADDR.24.llvm.ADDR+0xc802>
lea    0xc46a0(%rip),%rcx        # ADDR <anon.ADDR.2.llvm.ADDR+0x70a0>
lea    0xbd829(%rip),%r8        # ADDR <anon.ADDR.2.llvm.ADDR+0x230>
lea    0x5f(%rsp),%rdx
mov    $0x11,%esi
call   *0xc7379(%rip)        # ADDR <_DYNAMIC+0x2c8>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b5a>
mov    $0x3e,%esi
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b01>
mov    $0x3e,%esi
lea    0xbd81c(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x248>
mov    %rax,0x78(%rsp)
mov    %rbp,%rdi
mov    0x78(%rsp),%rdx
call   *0xc7291(%rip)        # ADDR <_DYNAMIC+0x208>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b5a>
mov    $0x8,%ebp
mov    $0x8,%esi
lea    0xc48fe(%rip),%rax        # ADDR <anon.ADDR.2.llvm.ADDR+0x7350>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1afc>
mov    $0x10,%eax
mov    %rax,0x48(%rsp)
mov    %r13,%rbx
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    $0x10,%eax
mov    %rax,0x48(%rsp)
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b4c>
mov    $0x10,%eax
mov    %rax,0x48(%rsp)
mov    %rbp,%rbx
mov    0x48(%rsp),%rdi
mov    %rbx,%rsi
call   *0xc72be(%rip)        # ADDR <_DYNAMIC+0x280>
ud2
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b64>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b64>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b64>
jmp    ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b64>
mov    %rax,%rbx
mov    0x30(%rsp),%r14
lea    0x140(%rsp),%rdi
call   ADDR <core::ptr::drop_glue::<expanse_trie::map::ExpanseMap>>
mov    0x80(%rsp),%rax
cmpq   $0x0,(%rax)
je     ADDR <instructions::map_insert::__iai_callgrind_wrapper_mod::map_insert+0x1b90>
mov    %r14,%rdi
call   *0xc7238(%rip)        # ADDR <free@GLIBC_2.2.5>
mov    %rbx,%rdi
call   ADDR <_Unwind_Resume@plt>
call   *0xc7292(%rip)        # ADDR <_DYNAMIC+0x298>
int3
int3
