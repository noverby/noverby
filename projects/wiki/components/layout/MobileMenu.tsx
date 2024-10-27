import { Badge, IconButton, Paper, Slide, Stack } from '@mui/material';
import { useApps } from 'hooks';
import { IconId } from 'mime';
import { useEffect, useState } from 'react';

const MobileMenu = ({
  openDrawer,
  setOpenDrawer,
}: {
  openDrawer: boolean;
  setOpenDrawer: (val: boolean) => void;
}) => {
  const [scrollPosition, setScrollPosition] = useState(0);
  const [show, setShow] = useState(true);
  const apps = useApps();

  useEffect(() => {
    const scroll = document.querySelector('#scroll');
    scroll?.addEventListener('scroll', handleScroll);
    return () => scroll?.removeEventListener('scroll', handleScroll);
  }, [scrollPosition]);

  const handleScroll = (event: Event) => {
    const newScrollPosition = (event.target as HTMLDivElement)?.scrollTop;

    if (Math.abs(scrollPosition - newScrollPosition) > 4) {
      setShow(scrollPosition > newScrollPosition);
    }
    setScrollPosition(newScrollPosition);
  };

  return (
    <Slide direction="left" in={show}>
      <Paper
        sx={{
          borderRadius: '20px 0px 0px 20px',
          position: 'fixed',
          bottom: (t) => t.spacing(9),
          right: (t) => t.spacing(0),
        }}
        elevation={1}
      >
        <Stack direction="row">
          {apps.map((app) => (
            <IconButton
              aria-label={app.name}
              key={app.mimeId}
              color={app.active ? 'primary' : undefined}
              onClick={app.onClick}
            >
              <Badge
                invisible={!app.notifications}
                color="primary"
                variant="dot"
              >
                <IconId mimeId={app.mimeId} />
              </Badge>
            </IconButton>
          ))}
        </Stack>
      </Paper>
    </Slide>
  );
};

export default MobileMenu;
