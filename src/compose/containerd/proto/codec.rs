use std::marker::PhantomData;

use prost::Message;
use tonic::codec::{BufferSettings, Codec, DecodeBuf, Decoder, EncodeBuf, Encoder};
use tonic::Status;

#[derive(Debug, Clone)]
pub struct ProstCodec<T, U> {
    _pd: PhantomData<(T, U)>,
}

impl<T, U> ProstCodec<T, U> {
    pub fn new() -> Self {
        Self { _pd: PhantomData }
    }
}

impl<T, U> Default for ProstCodec<T, U> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, U> Codec for ProstCodec<T, U>
where
    T: Message + Send + 'static,
    U: Message + Default + Send + 'static,
{
    type Encode = T;
    type Decode = U;
    type Encoder = ProstEncoder<T>;
    type Decoder = ProstDecoder<U>;

    fn encoder(&mut self) -> Self::Encoder {
        ProstEncoder {
            _pd: PhantomData,
            buffer_settings: BufferSettings::default(),
        }
    }

    fn decoder(&mut self) -> Self::Decoder {
        ProstDecoder {
            _pd: PhantomData,
            buffer_settings: BufferSettings::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProstEncoder<T> {
    _pd: PhantomData<T>,
    buffer_settings: BufferSettings,
}

impl<T: Message> Encoder for ProstEncoder<T> {
    type Item = T;
    type Error = Status;

    fn encode(&mut self, item: Self::Item, buf: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
        item.encode(buf).expect("Message only errors if not enough space");
        Ok(())
    }

    fn buffer_settings(&self) -> BufferSettings {
        self.buffer_settings
    }
}

#[derive(Debug, Clone)]
pub struct ProstDecoder<U> {
    _pd: PhantomData<U>,
    buffer_settings: BufferSettings,
}

impl<U: Message + Default> Decoder for ProstDecoder<U> {
    type Item = U;
    type Error = Status;

    fn decode(&mut self, buf: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        let item = Message::decode(buf)
            .map(Option::Some)
            .map_err(|error| Status::internal(error.to_string()))?;
        Ok(item)
    }

    fn buffer_settings(&self) -> BufferSettings {
        self.buffer_settings
    }
}
