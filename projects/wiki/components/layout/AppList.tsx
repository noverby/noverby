import {
  Badge,
  BottomNavigation,
  BottomNavigationAction,
} from '@mui/material';
import { useApps } from 'hooks';
import { IconId } from 'mime';
import { alpha } from '@mui/system';

const RailNagivation = ({
  value,
  children,
}: {
  value?: string;
  children: JSX.Element[];
}) => {
  return (

      <BottomNavigation
        showLabels
        value={value}
        sx={{

          flexDirection: 'column',
          justifyContent: 'flex-start',
          height: 'unset',
          width: 72,
          padding: `8px 0px`,
          position: 'fixed',
          top: 0,
          bottom: 0,
          left: 0,
          backgroundColor:  t => alpha(t.palette.primary.main, 0.08),
        }}
      >
        {children}
      </BottomNavigation>
  );
};

const RailAction = ({
  label,
  icon,
  value,
  onClick,
  ...props
}: {
  label: string;
  icon: JSX.Element;
  value: string;
  onClick: () => Promise<void>;
}) => {
  return (
    <BottomNavigationAction
      label={label}
      icon={icon}
      value={value}
      onClick={onClick}
      {...props}
      sx={{
        maxWidth: 72,
        minWidth: 72,
        minHeight: 72,
        maxHeight: 72,
        padding: '14px 8px',
        '&$iconOnly': {
          paddingTop: 24,
          paddingBottom: 10,
        },
        '&$selected': {
          paddingTop: 14,
        },
        '& .MuiBottomNavigationAction-label': {
          paddingTop: 0,
          '&$selected': {
            fontSize: 14,
          },
          '&$iconOnly': {},
        },
      }}
    />
  );
};

const AppList = () => {
  const apps = useApps();
  return (
    <RailNagivation value={apps.find((app) => app.active)?.mimeId}>
      {apps.map((app) => (
        <RailAction
          key={app.mimeId}
          label={app.name}
          value={app.mimeId}
          onClick={app.onClick}
          icon={
            <Badge invisible={!app.notifications} color="primary" variant="dot">
              <IconId mimeId={app.mimeId} />
            </Badge>
          }
        />
      ))}
    </RailNagivation>
  );
};

export default AppList;
