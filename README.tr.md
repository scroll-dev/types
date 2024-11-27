# Title

[Readme in Turkish](README.tr.md)

# 📜 scroll-prover 📜
[![Birim Testi](https://github.com/scroll-tech/scroll-prover/actions/workflows/unit_test.yml/badge.svg)](https://github.com/scroll-tech/scroll-prover/actions/workflows/unit_test.yml)
![sorunlar](https://img.shields.io/github/issues/scroll-tech/scroll-prover)

## Kullanım

### Önkoşul

svm-rs](https://github.com/alloy-rs/svm-rs) aracılığıyla `0.8.19` sürümlü Solidity derleyicisi `solc` yükleyin:
``shell
cargo install svm-rs
svm yükleme 0.8.19
```

Test izlerinin git-alt modülünü getir
```Kabuk
git submodule init
git submodule update --checkout
```

Tüm kurulum parametrelerini indirin, derece `20`, `24` ve `26` [config](https://github.com/scroll-tech/scroll-prover/tree/main/integration/configs) içinde kullanılır.
Yalnızca `26` derecesindeki parametreleri indirebilir, ancak performansı etkileyebilir (parametreleri küçültürken).
```shell
make download-setup -e degree=20
make download-setup -e degree=24
make download-setup -e degree=26
```
Veya indirmek için başka bir derece ve hedef dizin belirtin.
```kabuk
# Varsayılan olarak `degree=26` ve `params_dir=./integration/test_params`.
make download-setup -e degree=DEGREE params_dir=PARAMS_DIR
```

### Test

make test-chunk-prove` ve `make test-agg-prove` scroll-prover'ın çok seviyeli devre kısıtlama sistemi için ana test girdileridir. Geliştiriciler bu testlerin kodlarını okuyarak sistemin nasıl çalıştığını anlayabilirler.

Ve başka testler de vardır:
- Birinci seviye devreyi test etmek için `make test-inner-prove` kullanılabilir.
- Son iki seviyeyi test etmek için `make test-batch-prove` kullanılabilir.
- `make test-batches-with-each-chunk-num-prove` farklı yığın numaraları ile yığın kanıtlamayı test etmek için kullanılabilir.

### İkililer

İkili dosyaları yerel olarak çalıştırmak için aşağıdaki komutu kullanabilirsiniz.

Bağlama sorunlarıyla karşılaşırsanız şu komutu çalıştırmanız gerekebilir
```Kabuk
cp `find ./target/release/ | grep libzktrie.so` /usr/local/lib/
```
zktrielib'i bağlayıcınızın bulabileceği bir yola taşımak için.

Yığın kanıtı oluşturmak için zkevm prover'ı çalıştırın (çalışma dizini `./integration`)
```Kabuk
cargo build --release --bin zkevm_prove

./target/release/zkevm_prove --help
```
Argümanları şu şekilde belirtebilir
```Kabuk
# Kanıt verileri `./integration/proof_data` adresine kaydedilecektir.
export OUTPUT_DIR="proof_data"

# Params dosyası `./integration/test_params` içinde bulunmalıdır.
cargo run --release --bin zkevm_prove -- --params=test_params --trace=tests/extra_traces/new.json
```

Yığın kanıtını doğrulamak için zkevm doğrulayıcıyı çalıştırın (çalışma dizini `./integration`)
```Kabuk
cargo build --release --bin zkevm_verify

./target/release/zkevm_verify --help
```
Argümanları şu şekilde belirtebilir
```Kabuk
cargo run --release --bin zkevm_verify -- --params=test_params --proof=proof_data
```

### Komut Dosyaları

- DB için okuma erişiminiz varsa, toplu testler için tam kanıt oluşturmak üzere komut çalıştırabilirsiniz:
```
export DB_HOST=
export DB_USER=
export DB_NAME=

sh scripts/gen_full_chunk_proofs.sh BATCH_INDEX
```

### Dockers

- `docker/chunk-prover` GPU chunk-prover oluşturmak ve çalıştırmak için kullanılır.
- `docker/mock-testnet` GPU mock-testnet (inner-prove veya chunk-prove) oluşturmak ve çalıştırmak için kullanılır.

### Doğrulayıcı Sözleşmesi

Doğrulayıcı sözleşmesinin hem YUL hem de bayt kodu, toplama testleri çalıştırılırken oluşturulabilir (`make test-agg-prove`). Toplama testleri çalıştırıldıktan sonra, scroll-prover'ın `integration` klasöründe yeni bir klasör oluşturulur ve `agg_tests_output_multi_DATE_TIME` olarak adlandırılır. Aşağıdaki dosyaları içerir:

- Yığın protokolü: `chunk_chunk_0.protocol`
- Yığın VK: `vk_chunk_0.vkey`
- Toplu VK: `vk_batch_agg.vkey`
- Doğrulayıcı YUL kaynak kodu: `evm_verifier.yul`
- Doğrulayıcı bayt kodu: `evm_verifier.bin`

YUL kaynak kodu params, VK ve num kanıt örneği ile oluşturulur, snark-verifier'da [gen_evm_verifier function](https://github.com/scroll-tech/snark-verifier/blob/develop/snark-verifier-sdk/src/evm_api.rs#L121)'a başvurabilir.

Doğrulayıcı bayt kodu YUL kaynak kodundan derlenir, Solidity derleyicisini (yukarıda belirtildiği gibi `0.8.19`) belirtilen parametrelerle komut satırına çağırır, snark-verifier'da [compile_yul function](https://github.com/scroll-tech/snark-verifier/blob/develop/snark-verifier/src/loader/evm/util.rs#L107) işlevine başvurabilir.

## Lisans

Aşağıdakilerden biri altında lisanslanmıştır

- Apache Lisansı, Sürüm 2.0, ([LICENSE-APACHE](LICENSE-APACHE) veya http://www.apache.org/licenses/LICENSE-2.0)
- MIT lisansı ([LICENSE-MIT](LICENSE-MIT) veya http://opensource.org/licenses/MIT)

senin seçimin.
