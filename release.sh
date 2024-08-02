IN=integration/outputs/e2e_tests
OUT=release-v0.12.0

mkdir -p $OUT

# copy snark protocol and verification keys
cp $IN/chunk_chunk_0.protocol $OUT/chunk.protocol
cp $IN/vk_chunk_0.vkey $OUT/vk_chunk.vkey
cp $IN/vk_batch_agg.vkey $OUT/vk_batch.vkey
cp $IN/vk_bundle_recursion.vkey $OUT/vk_bundle.vkey

# copy verifier contract binary
cp $IN/evm_verifier.bin $OUT/evm_verifier.bin
cp $IN/evm_verifier.yul $OUT/evm_verifier.yul

# copy public input and proof for the outermost circuit (recursive bundler)
cp $IN/pi_bundle_recursion.data $OUT/pi.data
cp $IN/proof_bundle_recursion.data $OUT/proof.data

# dump the preprocessed digest into a separate hex file.
# preprocessed digest is the first 32 bytes of the public input.
xxd -l 32 -p $OUT/pi.data | tr -d '\n' | awk '{gsub("%", ""); print}' > $OUT/preprocessed_digest.hex

# copy config values for each proving layer
cp ./integration/configs/* $OUT

cd $OUT; sha256sum * > sha256sum; cd ..

aws --profile default s3 cp $OUT s3://circuit-release/$OUT --recursive
