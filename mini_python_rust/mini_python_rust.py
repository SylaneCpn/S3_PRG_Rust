import sys
import ctypes

print()
native_lib_name=sys.argv[1]
print('• loading', native_lib_name)
native_lib=ctypes.CDLL(native_lib_name)

print()
function_1_name='say_hello'
print('• accessing', function_1_name)
function_1=native_lib[function_1_name]
print('• calling', function_1_name)
function_1()


print()
function_2_name='compute'
print('• accessing', function_2_name)
function_2=native_lib[function_2_name]
function_2.argtypes=[ctypes.c_double, ctypes.c_double, ctypes.c_char_p]
function_2.restype=ctypes.c_double
print('• calling', function_2_name)
first = 2
second = 3
op = "div".encode()
print(f'• args : first = {first} , second = {second} , op = {op}')
res = function_2(first,second,op)
print(f'• result = {res} ')

print()
function_3_name='transform'
print('• accessing', function_3_name)
function_3=native_lib[function_3_name]
function_3.argtypes=[ctypes.c_void_p , ctypes.c_size_t]
function_3.restype=None
print('• calling', function_3_name)

N = 10
array_type= ctypes.c_double * N
arr = []
for i in range(N):
    arr.append(i + 0.1 * i + 1.2)
values = array_type(*arr)

print(f'• args :  values= {list(values)} , len={N}')
function_3(values,N)
print(f'• after transform : values = {list(values)} ')