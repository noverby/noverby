{pkgs, ...}: {
  security.wrappers.cloud-hypervisor = {
    owner = "root";
    group = "kvm";
    capabilities = "cap_net_admin+ep";
    source = "${pkgs.cloud-hypervisor}/bin/cloud-hypervisor";
  };
}
