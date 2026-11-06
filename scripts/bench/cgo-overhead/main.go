package main

/*
#include <stdint.h>
extern void goCallback(int);
static int noop(int x){ return x+1; }
static void callback_loop(int n){ for(int i=0;i<n;i++) goCallback(i); }
static uint64_t c_loop(int n){ uint64_t s=0; for(int i=0;i<n;i++) s+=noop(i); return s; }
*/
import "C"
import ("fmt"; "time")

var sink int
//export goCallback
func goCallback(i C.int){ sink += int(i) }

func main(){
	const n = 10_000_000
	t := time.Now(); for i := 0; i < n; i++ { C.noop(C.int(i)) }
	fmt.Printf("Go->C call: %.1f ns\n", float64(time.Since(t).Nanoseconds())/n)
	t = time.Now(); C.callback_loop(n)
	fmt.Printf("C->Go callback: %.1f ns\n", float64(time.Since(t).Nanoseconds())/n)
	t = time.Now(); C.c_loop(n)
	fmt.Printf("C-only loop: %.2f ns/iter\n", float64(time.Since(t).Nanoseconds())/n)
}
