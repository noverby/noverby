import dynamic from "next/dynamic";
import * as THREE from "three";
import { useRouter } from "next/router";
import { useMediaQuery } from "react-responsive";
import { useState } from "react";

const ForceGraph3D = dynamic(() => import("react-force-graph-3d"), {
  ssr: false,
});

type Node = {
  id: string;
  url?: string;
  icon?: string;
  color?: string;
  desc?: string;
};
type Link = { source?: string; target?: string; value?: number };
type GraphData = {
  nodes: Node[];
  links: Link[];
};

const nodes = [
  {
    id: "Niclas Overby",
    desc: "Niclas Overby Ⓝ",
    icon: "me.png",
  },
  {
    id: "Commerce",
    desc: "Commerce",
    icon: "commerce.png",
    color: "#45b1e8",
  },
  {
    id: "Improve",
    desc: "Improve",
    icon: "improve.png",
    color: "#7fff00",
  },
  {
    id: "Connect",
    desc: "Connect",
    icon: "connect.png",
    color: "#e34234",
  },
  {
    id: "Immerse",
    desc: "Immerse",
    color: "#ff7f50",
    icon: "immerse.png",
  },
  {
    id: "Give",
    desc: "Give",
    icon: "give.png",
    color: "#6a5acd",
  },
  {
    id: "Fediverse",
    desc: "Fediverse\nInfo",
    icon: "fediverse.png",
    color: "#000000",
    url: "https://fediverse.info",
  },
  {
    id: "LinkedIn",
    desc: "LinkedIn\nProfile",
    icon: "linkedin.png",
    url: "https://www.linkedin.com/in/niclasoverby",
  },
  {
    id: "PixelFed",
    desc: "PixelFed\nProfile",
    icon: "pixelfed.png",
    url: "https://pixelfed.social/niclasoverby",
  },
  {
    id: "Mail",
    desc: "Send Mail",
    icon: "mail.png",
    url: "mailto:niclas@overby.me",
  },
  {
    id: "Matrix",
    desc: "Matrix\nProfile",
    icon: "matrix.png",
    url: "https://matrix.to/#/@niclasoverby:beeper.com",
  },
  {
    id: "Signal",
    desc: "Signal\nProfile",
    icon: "signal.png",
    url: "https://signal.me/#eu/BKjgrHvQhqgDPpy9p2VfcfVj6yx0mJtVGOX8GQ_2htxhX7cDxhREVad8oWL1qAMj",
  },
  {
    id: "Spotify",
    desc: "Spotify\nProfile",
    icon: "spotify.png",
    url: "https://open.spotify.com/user/1148979230?si=218d80965cb8458f",
  },
  {
    id: "GitHub",
    desc: "GitHub\nProfile",
    icon: "github.png",
    url: "https://github.com/noverby",
  },
  {
    id: "Codeberg",
    desc: "Codeberg\nProfile",
    icon: "codeberg.png",
    url: "https://codeberg.com/noverby",
  },
  {
    id: "Mastodon",
    desc: "Mastodon\nProfile",
    icon: "mastodon.png",
    url: "https://mas.to/@niclasoverby",
  },
  {
    id: "Bluesky",
    desc: "Bluesky\nProfile",
    icon: "bluesky.png",
    url: "https://bsky.app/profile/overby.me",
  },
  {
    id: "Radikale Venstre",
    desc: "Radikale Venstre\n(Political Effort)",
    icon: "radikale.png",
    url: "https://www.radikale.dk",
  },
  {
    id: "Aivero",
    desc: "Aivero\n(Ex-company)",
    icon: "aivero.png",
    url: "https://www.aivero.com",
  },
  {
    id: "Factbird",
    desc: "Factbird\n(Commercial Effort)",
    icon: "factbird.png",
    url: "https://www.factbird.com",
  },
  {
    id: "Wikipedia",
    desc: "Wikipedia\nProfile",
    icon: "wikipedia.png",
    url: "https://en.wikipedia.org/wiki/User:Niclas_Overby",
  },
  {
    id: "HappyCow",
    desc: "HappyCow\nProfile",
    icon: "happycow.png",
    url: "https://www.happycow.net/members/profile/niclasoverby",
  },
  {
    id: "Lemmy",
    desc: "Lemmy\nProfile",
    icon: "lemmy.png",
    url: "https://lemmy.world/u/noverby",
  },
  {
    id: "NeoDB",
    desc: "NeoDB\nProfile",
    icon: "neodb.png",
    url: "https://neodb.social/users/niclasoverby",
  },
];

const links = [
  { source: "Niclas Overby", target: "Commerce" },
  { source: "Niclas Overby", target: "Improve" },
  { source: "Niclas Overby", target: "Connect" },
  { source: "Niclas Overby", target: "Immerse" },
  { source: "Niclas Overby", target: "Give" },
  { source: "Connect", target: "Matrix" },
  { source: "Connect", target: "Mail" },
  { source: "Connect", target: "LinkedIn" },
  { source: "Connect", target: "Mastodon" },
  { source: "Connect", target: "PixelFed" },
  { source: "Connect", target: "Bluesky" },
  { source: "Connect", target: "Signal" },
  { source: "Commerce", target: "LinkedIn" },
  { source: "Commerce", target: "Aivero" },
  { source: "Commerce", target: "Factbird" },
  { source: "Commerce", target: "GitHub" },
  { source: "Immerse", target: "PixelFed" },
  { source: "Immerse", target: "Spotify" },
  { source: "Immerse", target: "NeoDB" },
  { source: "Immerse", target: "Wikipedia" },
  { source: "Immerse", target: "HappyCow" },
  { source: "Immerse", target: "Lemmy" },
  { source: "Give", target: "Wikipedia" },
  { source: "Give", target: "Codeberg" },
  { source: "Give", target: "Radikale Venstre" },
  { source: "Give", target: "HappyCow" },
  { source: "Improve", target: "Codeberg" },
  { source: "Improve", target: "NeoDB" },
  { source: "PixelFed", target: "Fediverse" },
  { source: "Mastodon", target: "Fediverse" },
  { source: "Lemmy", target: "Fediverse" },
  { source: "NeoDB", target: "Fediverse" },
];

const graphData: GraphData = {
  nodes,
  links,
};

const Graph = () => {
  const router = useRouter();

  return (
    <ForceGraph3D
      graphData={graphData}
      nodeLabel={(node: any) => {
        return `<b style="white-space: pre; color: #ffffff; display: flex; text-align: center; font-size: 30px; text-shadow: 0 0 5px #000000, 2px 2px 18px #ff0072;">${node.desc}</b>`;
      }}
      backgroundColor="#222222"
      linkDirectionalParticles={2}
      linkDirectionalParticleWidth={1}
      onNodeClick={(node: any) => node.url && router.push(node.url)}
      nodeThreeObject={(node: any) => {
        if (!node.color) {
          const imgTexture = new THREE.TextureLoader().load(
            `icons/${node.icon}`,
          );
          const material = new THREE.SpriteMaterial({ map: imgTexture });
          const sprite = new THREE.Sprite(material);
          const size = node.id === "Niclas Overby" ? 40 : 18;
          sprite.scale.set(size, size, 0);

          return sprite;
        } else {
          const group = new THREE.Group();
          const imgTexture = new THREE.TextureLoader().load(
            `icons/${node.icon}`,
          );
          const material = new THREE.SpriteMaterial({ map: imgTexture });
          const icon = new THREE.Sprite(material);
          icon.scale.set(20, 20, 0);
          group.add(
            icon,
            new THREE.Mesh(
              new THREE.SphereGeometry(15),
              new THREE.MeshLambertMaterial({
                color: node.color,
                transparent: true,
                opacity: 0.4,
              }),
            ),
          );

          return group;
        }
      }}
    />
  );
};

export default Graph;
