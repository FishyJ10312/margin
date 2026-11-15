// A read-only bootstrap deadline must not retry mutations or change IPC globals.
export async function startupDeadline<T>(work:Promise<T>,milliseconds=20000):Promise<T>{
 let timer:ReturnType<typeof setTimeout>|undefined;
 try{return await Promise.race([work,new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new Error('本机连接暂未响应，请点击重新加载。')),milliseconds);})]);}
 finally{clearTimeout(timer);}
}
