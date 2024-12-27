import {
  Avatar,
  ImageList,
  ImageListItem,
  ImageListItemBar,
  useMediaQuery,
} from '@mui/material';
import { Node, useLink, useScreen, useFiles } from 'hooks';
import { IconId } from 'mime';
import { Image } from 'comps';
import { Box } from '@mui/system';
import { useUserId } from '@nhost/nextjs';

const CandidateList = ({ node }: { node: Node }) => {
  const link = useLink();
  const screen = useScreen();
  const largeScreen = useMediaQuery('(min-width:1200px)');
  const userId = useUserId();

  const children = node.useQuery()?.children({
    where: {
      _and: [
        { mimeId: { _eq: 'vote/candidate' } },
        {
          _or: [
            { mutable: { _eq: false } },
            { ownerId: { _eq: userId } },
            { members: { nodeId: { _eq: userId } } },
          ],
        },
      ],
    },
  });

  const imageIds = children?.map((child) => child.data()?.image);
  const images = useFiles({ fileIds: imageIds, image: true });

  if (screen) return null;

  return (
    <ImageList cols={largeScreen ? 2 : 1} sx={{ m: 0 }}>
      {children?.map(({ id, name, key }, index) =>
        !id ? null : (
          <ImageListItem
            key={id ?? 0}
            sx={{ borderRadius: '70px', cursor: 'pointer' }}
            onClick={() => link.push([key!])}
          >
            {images?.[index] ? (
              <Image
                alt="Billede for indhold"
                layout="fill"
                src={images?.[index]}
              />
            ) : (
              <Box sx={{ height: '50px' }}></Box>
            )}
            <ImageListItemBar
              sx={{ borderRadius: '20px', color: 'white' }}
              title={name}
              position="top"
              actionPosition="left"
              actionIcon={
                <Avatar
                  sx={{
                    bgcolor: 'secondary.main',
                    m: 1,
                    height: 32,
                    width: 32,
                  }}
                >
                  <IconId mimeId="vote/candidate" />
                </Avatar>
              }
            />
          </ImageListItem>
        )
      ) ?? []}
    </ImageList>
  );
};

export default CandidateList;
