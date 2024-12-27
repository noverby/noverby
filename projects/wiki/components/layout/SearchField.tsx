import { Menu, Search, SearchOff } from '@mui/icons-material';
import {
  Autocomplete,
  Avatar,
  CircularProgress,
  IconButton,
  InputBase,
  InputBaseProps,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Paper,
  Stack,
  Tooltip,
  useMediaQuery,
} from '@mui/material';
import { order_by, resolve } from 'gql';
import { useLink, usePath, useSession } from 'hooks';
import { MimeIconId, Breadcrumbs, Bar, UserMenu } from 'comps';
import {
  ForwardedRef,
  forwardRef,
  Fragment,
  startTransition,
  useEffect,
  useRef,
  useState,
  useTransition,
} from 'react';
import { fromId } from 'core/path';

const StyledInputRef = (
  props: InputBaseProps,
  ref?: ForwardedRef<HTMLInputElement>
) => (
  <InputBase
    {...props}
    ref={ref}
    sx={{
      '& .MuiInputBase-input': {
        '&::placeholder': {
          opacity: 1,
        },
        width: '100%',
        padding: (t) => t.spacing(1.5, 1, 1.5, 2),
        transition: (t) => t.transitions.create('width'),
        color: 'common.white',
      },
    }}
  />
);

const StyledInput = forwardRef(StyledInputRef);

const MenuAvatar = ({
  openDrawer,
  setOpenDrawer,
}: {
  openDrawer: boolean;
  setOpenDrawer: (val: boolean) => void;
}) => {
  return (
    <Tooltip title="Menu">
      <IconButton onClick={() => startTransition(() => setOpenDrawer(true))}>
        <Avatar sx={{ bgcolor: 'secondary.main' }}>
          <Menu />
        </Avatar>
      </IconButton>
    </Tooltip>
  );
};

const SearchField = ({
  openDrawer,
  setOpenDrawer,
}: {
  openDrawer: boolean;
  setOpenDrawer: (val: boolean) => void;
}) => {
  const link = useLink();
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [searchMode, setSearchMode] = useState(false);
  const [input, setInput] = useState('');
  const [session, setSession] = useSession();
  const [open, setOpen] = useState(false);
  const [options, setOptions] = useState<
    {
      id?: string;
      name?: string;
      parent: { name?: string };
      context?: boolean;
    }[]
  >([]);
  const [isLoading, setIsLoading] = useState(false);
  const [selected, setSelected] = useState<string>();
  const [selectIndex, setSelectIndex] = useState(0);
  const [_, startTransition] = useTransition();
  const path = usePath();
  const largeScreen = useMediaQuery('(min-width:1200px)');

  const handleContextSelect = async (id: string) => {
    const prefix = await resolve(({ query }) => {
      const node = query.node({ id });
      return {
        id: node?.id,
        name: node?.name ?? '',
        mime: node?.mimeId!,
        key: node?.key,
      };
    });

    const path = await fromId(id);
    startTransition(() => {
      setSession({
        prefix: {
          ...prefix,
          path,
        },
      });
      link.id(id);
    });
  };

  const goto = async (id?: string, context?: boolean) => {
    if (!id) return;
    if (context) {
      await handleContextSelect(id);
    } else {
      await link.id(id);
    }
    setOpen(false);
    setInput('');
  };

  const search = async (name: string) => {
    setIsLoading(true);
    const nodes = await resolve(({ query }) =>
      query
        .nodes({
          where: {
            _and: [
              {
                mime: {
                  _or: [{ hidden: { _eq: false } }, { context: { _eq: true } }],
                },
              },
              path.length == 0
                ? {}
                : { contextId: { _eq: session?.prefix?.id } },
              { name: { _ilike: `%${name}%` } },
              { parent: { id: { _is_null: false } }}
            ],
          },
          limit: name ? 10 : 0,
          order_by: [{ mime: { context: order_by.desc } }],
        })
        .map(({ id, name, mimeId, parent, mime }) => ({
          id,
          name,
          mimeId,
          parent: { name: parent?.name },
          context: mime?.context,
        }))
    );
    setOptions(nodes);
    setIsLoading(false);
  };

  useEffect(() => {
    startTransition(() => {
      setSelected(options?.[selectIndex]?.id);
    });
  }, [JSON.stringify(options), selectIndex]);

  useEffect(() => {
    const handleKeyPress = (event: KeyboardEvent) => {
      if (event.ctrlKey === true && event.key === 'k') {
        event.preventDefault();
        startTransition(() => setSearchMode(!searchMode));
      }
    };

    document.addEventListener('keydown', handleKeyPress);

    return () => {
      document.removeEventListener('keydown', handleKeyPress);
    };
  }, [searchMode]);

  const autocomplete = (
    <Autocomplete
      sx={{ width: '100%', height: '100%' }}
      open={open}
      inputValue={input}
      onOpen={() => setOpen(true)}
      onClose={() => setOpen(false)}
      onKeyDown={(e) =>
        startTransition(() => {
          if (e.key === 'ArrowDown')
            setSelectIndex(
              options.length - 1 > selectIndex ? selectIndex + 1 : selectIndex
            );
          if (e.key === 'ArrowUp')
            setSelectIndex(selectIndex > 0 ? selectIndex - 1 : selectIndex);
          if (e.key === 'Enter') {
            setSearchMode(false);
            if (options?.[selectIndex]?.id)
              goto(options?.[selectIndex]?.id, options?.[selectIndex]?.context);
          }
          const scroll = document.querySelector(
            `#o${options?.[selectIndex]?.id}`
          );
          scroll?.scrollIntoView();
        })
      }
      noOptionsText="Intet resultat"
      loadingText="Loader..."
      isOptionEqualToValue={(option, value) => option.id === value.id}
      getOptionLabel={(option) => option.name ?? ''}
      options={options}
      loading={isLoading}
      onInputChange={(e, value) => {
        if (e) {
          setInput(value);
          setSelectIndex(0);
          search(value);
        }
      }}
      renderOption={(props, option) => (
        <Fragment key={option.id}>
          <ListItemButton
            key={`o${option.id}`}
            selected={selected === option.id}
            onClick={() => {
              setSearchMode(false);
              goto(option.id, option.context);
            }}
          >
            {option.id && (
              <ListItemIcon sx={{ color: 'secondary.main' }}>
                <MimeIconId id={option?.id} />
              </ListItemIcon>
            )}
            <ListItemText secondary={option.parent?.name}>
              {option.name}
            </ListItemText>
          </ListItemButton>
        </Fragment>
      )}
      PaperComponent={(props) => (
        <Paper {...props} sx={{ height: '100%' }}>
          <List dense>{props.children}</List>
        </Paper>
      )}
      renderInput={(params) => (
        <Bar ref={params.InputProps.ref}>
          <Stack direction="row" spacing={-1}>
            {!largeScreen && (
              <MenuAvatar
                openDrawer={openDrawer}
                setOpenDrawer={setOpenDrawer}
              />
            )}

            <StyledInput
              inputRef={(input) => {
                // eslint-disable-next-line functional/immutable-data
                inputRef.current = input;
              }}
              autoFocus
              placeholder="Søg"
              fullWidth
              inputProps={{
                ...params.inputProps,
                endadornment: isLoading ? (
                  <CircularProgress color="inherit" size={20} />
                ) : null,
              }}
            />
            <IconButton
              aria-label='Afslut Søgefelt'
              onClick={() => startTransition(() => setSearchMode(false))}
            >
              <Avatar sx={{ bgcolor: 'primary.main' }}>
                <SearchOff />
              </Avatar>
            </IconButton>
            <UserMenu avatar />
          </Stack>
        </Bar>
      )}
    />
  );

  return searchMode ? (
    autocomplete
  ) : (
    <Bar>
      <Stack direction="row" spacing={-1}>
        {!largeScreen && (
          <MenuAvatar openDrawer={openDrawer} setOpenDrawer={setOpenDrawer} />
        )}
        <Breadcrumbs />
        <IconButton aria-label='Søg' onClick={() => startTransition(() => setSearchMode(true))}>
          <Avatar sx={{ bgcolor: 'primary.main' }}>
            <Search />
          </Avatar>
        </IconButton>
        <UserMenu avatar />
      </Stack>
    </Bar>
  );
};

export default SearchField;
