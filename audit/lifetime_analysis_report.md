# Табби Lifetime Audit

```shell
=== TABBY LIFETIME AUDIT ===
Дата: Sun Aug 10 18:37:29 MSK 2025

1. АВТОГЕНЕРИРОВАННЫЕ LIFETIMES ('life0, 'life1, etc.):

2. ЯВНЫЕ LIFETIMES ('a, 'b, etc.):
./crates/hash-ids/src/lib.rs:35:    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
./crates/ollama-api-bindings/src/completion.rs:23:    async fn generate<'a>(&'a self, prompt: &str, options: CompletionOptions) -> BoxStream<'a, String> {
./crates/http-api-bindings/src/completion/mistral.rs:70:    async fn generate<'a>(&'a self, prompt: &str, options: CompletionOptions) -> BoxStream<'a, String> {
./crates/http-api-bindings/src/completion/llama.rs:47:    async fn generate<'a>(&'a self, prompt: &str, options: CompletionOptions) -> BoxStream<'a, String> {
./crates/http-api-bindings/src/completion/openai.rs:67:    async fn generate<'a>(&'a self, prompt: &str, options: CompletionOptions) -> BoxStream<'a, String> {
./crates/http-api-bindings/src/rate_limit.rs:63:    async fn generate<'a>(&'a self, prompt: &str, options: CompletionOptions) -> BoxStream<'a, String> {
./crates/tabby-git/src/lib.rs:79:fn rev_to_commit<'a>(
./crates/tabby-git/src/lib.rs:82:) -> anyhow::Result<git2::Commit<'a>> {
./crates/tabby-git/src/grep/output.rs:25:    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
./crates/tabby-git/src/grep/output.rs:64:    pub fn negative_sink(&mut self) -> GrepNegativeMatchSink<'_> {
./crates/tabby-git/src/grep/output.rs:147:        mat: &grep::searcher::SinkMatch<'_>,
./crates/tabby-git/src/grep/output.rs:181:        context: &grep::searcher::SinkContext<'_>,
./crates/tabby-git/src/grep/output.rs:200:pub struct GrepNegativeMatchSink<'output> {
./crates/tabby-git/src/grep/output.rs:204:impl Sink for GrepNegativeMatchSink<'_> {
./crates/tabby-git/src/grep/output.rs:210:        _mat: &grep::searcher::SinkMatch<'_>,
./crates/tabby-git/src/serve_git.rs:17:fn resolve<'a>(
./crates/tabby-git/src/serve_git.rs:21:) -> anyhow::Result<Resolve<'a>> {
./crates/tabby-git/src/serve_git.rs:113:pub enum Resolve<'a> {
./crates/tabby-git/src/serve_git.rs:115:    File(Mime, Blob<'a>),
./crates/tabby-inference/src/decoding.rs:26:type CachedTrie<'a> = dashmap::mapref::one::Ref<'a, String, Trie<u8>>;

3. ANONYMOUS LIFETIMES ('_):
./crates/hash-ids/src/lib.rs:35:    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
./crates/tabby-git/src/grep/output.rs:25:    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
./crates/tabby-git/src/grep/output.rs:64:    pub fn negative_sink(&mut self) -> GrepNegativeMatchSink<'_> {
./crates/tabby-git/src/grep/output.rs:141:impl Sink for GrepMatchSink<'_, '_> {
./crates/tabby-git/src/grep/output.rs:147:        mat: &grep::searcher::SinkMatch<'_>,
./crates/tabby-git/src/grep/output.rs:181:        context: &grep::searcher::SinkContext<'_>,
./crates/tabby-git/src/grep/output.rs:204:impl Sink for GrepNegativeMatchSink<'_> {
./crates/tabby-git/src/grep/output.rs:210:        _mat: &grep::searcher::SinkMatch<'_>,
./crates/tabby-index-cli/src/timer.rs:15:impl OpenTimer<'_> {
./crates/tabby-index-cli/src/timer.rs:21:    pub fn open(&mut self, name: &'static str) -> OpenTimer<'_> {
./crates/tabby-index-cli/src/timer.rs:31:impl Drop for OpenTimer<'_> {
./crates/tabby-index-cli/src/timer.rs:63:    pub fn open(&mut self, name: &'static str) -> OpenTimer<'_> {
./crates/tabby-index/src/indexer.rs:67:        impl Stream<Item = JoinHandle<Option<TantivyDocument>>> + '_,
./crates/tabby-index/src/indexer.rs:140:    ) -> impl Stream<Item = JoinHandle<Result<TantivyDocument>>> + '_ {
./crates/tabby-index/src/indexer.rs:334:    pub fn iter_ids(&self) -> impl Stream<Item = (String, String)> + '_ {
./crates/tabby-common/src/config.rs:212:            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.' | '-' => c,
./crates/tabby-common/src/config.rs:213:            _ => '_',
./crates/tabby-common/src/config.rs:216:    sanitized.dedup_by(|a, b| *a == '_' && *b == '_');
./crates/tabby-common/src/terminal.rs:50:impl ToString for InfoMessage<'_> {
./crates/tabby/src/services/completion.rs:589:        async fn generate(&self, _prompt: &'_ str, _options: CompletionOptions) -> BoxStream<String> {

4. СТРУКТУРЫ С LIFETIMES:
./crates/tabby-git/src/grep/output.rs:136:pub struct GrepMatchSink<'output, 'a> {
./crates/tabby-git/src/grep/output.rs:200:pub struct GrepNegativeMatchSink<'output> {
./crates/tabby-inference/src/decoding.rs:73:pub struct StopCondition<'a> {
./crates/tabby-index-cli/src/timer.rs:8:pub struct OpenTimer<'a> {
./crates/tabby-common/src/terminal.rs:25:pub struct InfoMessage<'a> {
./crates/tabby-common/src/usage.rs:68:struct Payload<'a, T> {
./target/debug/build/cssparser-d03fb6271bd7a56d/out/tokenizer.rs:13:} # [derive (Clone)] pub struct Tokenizer < 'a > {
./target/debug/build/cssparser-94c3ad087203b673/out/tokenizer.rs:13:} # [derive (Clone)] pub struct Tokenizer < 'a > {
./target/debug/build/cssparser-afdcec19de2d6c79/out/tokenizer.rs:13:} # [derive (Clone)] pub struct Tokenizer < 'a > {
./ee/tabby-webserver/src/service/auth/testutils.rs:17:pub struct FakeLdapClient<'a> {

5. СТАТИСТИКА ПО ФАЙЛАМ:
     178
файлов содержат lifetimes
```
