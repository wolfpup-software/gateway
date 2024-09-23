# edit the following vars to create a custom key and cert 

curr_dir=`dirname $0`
target_key=$curr_dir/demo/self-signed-key.key
target_cert=$curr_dir/demo/self-signed-cert.crt
subject="/C=US/ST=CA/L=SF/O=Toshokan/OU=Education/CN=*.toshokan.org/emailAddress=taylor@toshokan.org"

openssl req -newkey rsa:4096 -x509 -sha256 \
    -days 365 -nodes \
    -keyout $target_key \
    -subj $subject \
    -out $target_cert
