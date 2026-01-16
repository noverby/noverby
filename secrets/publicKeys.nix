rec {
  noverby-ssh-ed25519 = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOachAYzBH8Qaorvbck99Fw+v6md3BeVtfL5PJ/byv4Cc";
  noverby-ssh-rsa = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQDVVn8JxJlkjZhyKTgkfv6fOh3PfDxaN28kstlNHqondU8pCpANVoIX8tXWeNhF5kvKrCNMW2/NTZVH8tY4a818MfHf9rHHAtFea3ZkQ7ji3hxyn/OzyNd54p4rRPp0MkrnQeg9sKg8RPf+8k5tK/B5nXdsGiTJ1C44yQAdDFstdTq+Sykd8ZZnDQf4fWRShZpbINUo0k+eWcgRnFeaSZS3yeXq9cMLcP/M8RG8WkDf50fXDGou8Qhpgib6GoYa7wtxJ4OBCtpfpBHmEjt9WNPPHRHLKeQeSRWgL2fQbpZ6gatsQLTNuu0Lux1uxXZksQKtnMUoOlMfjMlE8uVIvRW7 niclas@overby.me
";
  nitrokey3-fido2-hmac = "age1efazxe5tgepdv5czxzj5x844dj265dar4sej42qc8mjp2czulazslqutuw";
  all = [noverby-ssh-ed25519 noverby-ssh-rsa nitrokey3-fido2-hmac];
}
