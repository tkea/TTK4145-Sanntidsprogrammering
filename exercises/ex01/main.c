// gcc 4.7.2 +
// gcc -std=gnu99 -Wall -g -o helloworld_c helloworld_c.c -lpthread

#include <pthread.h>
#include <stdio.h>

#define NUM_COMPUTATIONS 1000*1000

static int i;

void* increment_i(){
    for(int x=0; x<NUM_COMPUTATIONS; x++) i++;
    return NULL;
}


void* decrement_i(){
    for(int x=0; x<NUM_COMPUTATIONS; x++) i--;
    return NULL;
}


int main(){
    pthread_t increment_thread, decrement_thread;
    pthread_create(&increment_thread, NULL, increment_i, NULL);
    pthread_create(&decrement_thread, NULL, decrement_i, NULL);

    pthread_join(increment_thread, NULL);
    pthread_join(decrement_thread, NULL);

    printf("Result: %d", i);
    return 0;
}
