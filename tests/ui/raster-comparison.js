import {inflateSync} from 'node:zlib';

// Decode Chromium's 8-bit RGB/RGBA PNG output without adding a runtime package.
function pixels(buffer) {
  let offset=8,width,height,channels; const compressed=[];
  while(offset<buffer.length) {
    const length=buffer.readUInt32BE(offset),type=buffer.toString('ascii',offset+4,offset+8),data=buffer.subarray(offset+8,offset+8+length);
    if(type==='IHDR') {width=data.readUInt32BE(0);height=data.readUInt32BE(4);if(data[8]!==8||![2,6].includes(data[9])||data[12]!==0)throw new Error('Unsupported screenshot PNG');channels=data[9]===2?3:4;}
    if(type==='IDAT')compressed.push(data);
    offset+=length+12;
  }
  const encoded=inflateSync(Buffer.concat(compressed)),stride=width*channels,decoded=Buffer.alloc(stride*height);
  for(let y=0;y<height;y++) {
    const filter=encoded[y*(stride+1)],start=y*stride;
    for(let x=0;x<stride;x++) {
      const left=x>=channels?decoded[start+x-channels]:0,above=y?decoded[start-stride+x]:0,corner=y&&x>=channels?decoded[start-stride+x-channels]:0;
      let predictor=0;
      if(filter===1)predictor=left;
      else if(filter===2)predictor=above;
      else if(filter===3)predictor=Math.floor((left+above)/2);
      else if(filter===4){const p=left+above-corner,a=Math.abs(p-left),b=Math.abs(p-above),c=Math.abs(p-corner);predictor=a<=b&&a<=c?left:b<=c?above:corner;}
      else if(filter!==0)throw new Error('Unsupported PNG filter');
      decoded[start+x]=(encoded[y*(stride+1)+1+x]+predictor)&255;
    }
  }
  return {width,height,channels,data:decoded};
}
export function compareRaster(left,right) {
  const a=pixels(left),b=pixels(right);
  if(a.width!==b.width||a.height!==b.height||a.channels!==b.channels)throw new Error('Screenshot dimensions differ');
  let changedPixels=0,maxChannelDelta=0;
  for(let offset=0;offset<a.data.length;offset+=a.channels) {
    let changed=false;
    for(let channel=0;channel<a.channels;channel++){const delta=Math.abs(a.data[offset+channel]-b.data[offset+channel]);if(delta)changed=true;maxChannelDelta=Math.max(maxChannelDelta,delta);}
    if(changed)changedPixels++;
  }
  return {byteIdentical:left.equals(right),changedPixels,maxChannelDelta,totalPixels:a.width*a.height};
}
