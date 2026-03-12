use boltffi::*;

#[export]
#[allow(async_fn_in_trait)]
pub trait AsyncFetcher: Send + Sync {
    async fn fetch_value(&self, key: i32) -> i32;
    async fn fetch_string(&self, input: String) -> String;
}

#[export]
pub async fn fetch_with_async_callback(fetcher: impl AsyncFetcher, key: i32) -> i32 {
    fetcher.fetch_value(key).await
}

#[export]
pub async fn fetch_string_with_async_callback(
    fetcher: impl AsyncFetcher,
    input: String,
) -> String {
    fetcher.fetch_string(input).await
}

#[export]
#[allow(async_fn_in_trait)]
pub trait AsyncOptionFetcher: Send + Sync {
    async fn find(&self, key: i32) -> Option<i64>;
}

#[export]
pub async fn invoke_async_option_fetcher(
    fetcher: impl AsyncOptionFetcher,
    key: i32,
) -> Option<i64> {
    fetcher.find(key).await
}
